import type { BoxState } from "./shared-core.js";

function joinEnglish(items: string[]): string {
  if (items.length <= 1) return items[0] || "";
  if (items.length === 2) return `${items[0]} and ${items[1]}`;
  return `${items.slice(0, -1).join(", ")}, and ${items[items.length - 1]}`;
}

export function formatNames(names: string[]): string {
  return joinEnglish(names.map((name) => `$n{${name}}`));
}

export function boxesByName(boxes: BoxState[]): Map<string, BoxState> {
  return new Map(boxes.map((box) => [box.name, box]));
}
