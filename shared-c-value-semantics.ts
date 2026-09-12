import type { BoxState, CTypeInfo } from "./shared-core-utils.js";
import { evaluateCExpression } from "./shared-c-interpreter.js";

const typeInfoCache = new Map<string, CTypeInfo>();

/** Asks the interpreter to parse a type and report its target-model metadata. */
export function inspectCType(type: string): CTypeInfo | null {
  const normalized = type.trim();
  if (!normalized) return null;
  const cached = typeInfoCache.get(normalized);
  if (cached) return cached;
  const evaluated = evaluateCExpression("0;", 0, `(${normalized})0`);
  if (evaluated.kind !== "ok") return null;
  typeInfoCache.set(normalized, evaluated.result.typeInfo);
  return evaluated.result.typeInfo;
}

/** Normalizes a user-entered literal for hint context without accepting expressions. */
export function normalizeBoxValueForContext(box: BoxState): BoxState {
  const raw = String(box.rawValue ?? box.value ?? "").trim();
  if (!raw) return { ...box, value: raw };
  const evaluated = evaluateCExpression("0;", 0, raw);
  if (evaluated.kind !== "ok" || !evaluated.result.valueLiteral) {
    return { ...box, value: raw };
  }
  return {
    ...box,
    value: evaluated.result.value,
    displayValue: evaluated.result.displayValue,
    exactValue: evaluated.result.exactValue,
    typeInfo: evaluated.result.typeInfo,
  };
}

interface CValueMatch {
  ok: boolean;
  normalized: string;
}

/** Compares lesson answers using C conversion rules rather than JS coercion. */
export function boxValueMatchesSpec(
  actual: BoxState,
  expected: BoxState,
): CValueMatch {
  const actualRaw = String(actual.rawValue ?? actual.value ?? "").trim();
  const expectedRaw = String(expected.value ?? "").trim();
  if (!actualRaw || !expectedRaw) {
    const ok = actualRaw === expectedRaw;
    return { ok, normalized: ok ? expectedRaw : "" };
  }

  const targetType = String(expected.type || actual.type || "").trim();
  const targetKind = expected.typeInfo?.kind ?? "unknown";
  if (targetKind === "pointer") {
    // A displayed address only has provenance inside the execution that issued it.
    const ok = actualRaw === expectedRaw;
    return { ok, normalized: ok ? expectedRaw : "" };
  }

  const entered = evaluateCExpression("0;", 0, actualRaw);
  if (entered.kind !== "ok" || !entered.result.valueLiteral) {
    return { ok: false, normalized: "" };
  }
  if (
    (targetKind === "floating" && entered.result.typeInfo.kind !== "floating") ||
    (targetKind === "integer" && entered.result.typeInfo.kind !== "integer")
  ) {
    return { ok: false, normalized: "" };
  }

  const cast = (raw: string) =>
    evaluateCExpression("0;", 0, `(${targetType})(${raw})`);
  const convertedActual = cast(actualRaw);
  const convertedExpected = cast(expectedRaw);
  if (convertedActual.kind !== "ok" || convertedExpected.kind !== "ok") {
    return { ok: false, normalized: "" };
  }
  const ok =
    convertedActual.result.value === convertedExpected.result.value &&
    convertedActual.result.exactValue === convertedExpected.result.exactValue;
  return { ok, normalized: ok ? convertedExpected.result.value : "" };
}
