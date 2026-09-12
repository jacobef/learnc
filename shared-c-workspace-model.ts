import type { BoxState, CTypeInfo } from "./shared-core-utils.js";
import { runCProgram } from "./shared-c-interpreter.js";
import { inspectCType } from "./shared-c-value-semantics.js";

interface WorkspaceAllocation {
  address: string;
  typeInfo: CTypeInfo;
  assumedType: string;
}

function addressOf(box: BoxState): string {
  return String(box.address ?? "").trim();
}

/** Rebuilds the user-facing pointer aliases from one workspace snapshot. */
export function resolveCBoxAliases(boxes: BoxState[]): BoxState[] {
  const resolved = boxes.map((box) => ({ ...box, aliases: [] as string[] }));
  const byAddress = new Map(
    resolved.filter((box) => addressOf(box)).map((box) => [addressOf(box), box]),
  );
  for (const pointer of resolved) {
    const pointerDepth = inspectCType(pointer.type)?.pointerDepth ?? 0;
    if (!pointer.name || pointerDepth === 0) continue;
    let address = String(pointer.value ?? "").trim();
    for (let level = 1; level <= pointerDepth; level += 1) {
      const target = byAddress.get(address);
      if (!target) break;
      target.aliases!.push(`${"*".repeat(level)}${pointer.name}`);
      address = String(target.value ?? "").trim();
    }
  }
  return resolved;
}

const addressSlotsByType = new Map<string, BoxState[]>();

function loadAddressSlots(type: string, count: number): BoxState[] {
  const cached = addressSlotsByType.get(type) ?? [];
  if (cached.length >= count) return cached;
  const declarations = Array.from(
    { length: count },
    (_, index) => `${type} __cboxes_address_slot_${index};`,
  ).join("\n");
  const run = runCProgram(declarations);
  const slots =
    run.kind === "ok"
      ? run.state.filter((box) => box.name.startsWith("__cboxes_address_slot_"))
      : [];
  addressSlotsByType.set(type, slots);
  return slots;
}

/**
 * Allocates a displayed address using the same size, alignment, and address
 * model as Rust. These addresses identify lesson objects; they are not host
 * pointers and must not be reconstructed with JavaScript arithmetic alone.
 */
export function allocateCWorkspaceObject(
  boxes: BoxState[],
  requestedType: string,
): WorkspaceAllocation | null {
  const enteredType = requestedType.trim();
  const enteredTypeInfo = enteredType ? inspectCType(enteredType) : null;
  const assumedType = enteredTypeInfo ? enteredType : "int";
  const requestedTypeInfo = enteredTypeInfo ?? inspectCType("int");
  if (!requestedTypeInfo) return null;

  const occupied = boxes.flatMap((box) => {
    const start = Number(addressOf(box));
    if (!Number.isFinite(start)) return [];
    const size = Math.max(
      1,
      inspectCType(String(box.type || ""))?.size ?? box.typeInfo?.size ?? 1,
    );
    return [{ start, end: start + size }];
  });
  const allocationFrontier = occupied.reduce(
    (frontier, range) => Math.max(frontier, range.end),
    Number.NEGATIVE_INFINITY,
  );

  let slotCount = Math.max(32, boxes.length * 2 + 8);
  for (let attempt = 0; attempt < 5; attempt += 1) {
    for (const slot of loadAddressSlots(assumedType, slotCount)) {
      const address = addressOf(slot);
      const start = Number(address);
      if (!address || !Number.isFinite(start)) continue;
      const typeInfo = slot.typeInfo ?? requestedTypeInfo;
      const end = start + Math.max(1, typeInfo.size ?? 1);
      const available = occupied.every(
        (range) => end <= range.start || start >= range.end,
      );
      if (start >= allocationFrontier && available) {
        return { address, typeInfo, assumedType };
      }
    }
    slotCount *= 2;
  }
  return null;
}
