import { evaluateCExpression } from "./shared-c-interpreter.js";
const typeInfoCache = new Map();
/** Asks the interpreter to parse a type and report its target-model metadata. */
export function inspectCType(type) {
    const normalized = type.trim();
    if (!normalized)
        return null;
    const cached = typeInfoCache.get(normalized);
    if (cached)
        return cached;
    // A compound literal, unlike a cast, can have any object type, arrays included.
    const evaluated = evaluateCExpression("0;", 0, `(${normalized}){0}`);
    if (evaluated.kind !== "ok")
        return null;
    typeInfoCache.set(normalized, evaluated.result.typeInfo);
    return evaluated.result.typeInfo;
}
/** Normalizes a user-entered literal for hint context without accepting expressions. */
export function normalizeBoxValueForContext(box) {
    const raw = String(box.rawValue ?? box.value ?? "").trim();
    if (!raw)
        return { ...box, value: raw };
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
/** Compares lesson answers using C conversion rules rather than JS coercion. */
// Infinity and NaN, which can't be written as C constants. They can be typed as
// the program state shows them, like Infinity, or as printf prints them, like inf.
function specialFloatingValue(raw) {
    const match = /^([+-]?)(inf|infinity|nan)$/i.exec(raw.trim());
    if (!match)
        return null;
    return match[2].toLowerCase() === "nan" ? "nan" : `${match[1] === "-" ? "-" : ""}inf`;
}
export function boxValueMatchesSpec(actual, expected) {
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
    if (targetKind === "floating") {
        const actualSpecial = specialFloatingValue(actualRaw);
        const expectedSpecial = specialFloatingValue(expectedRaw);
        if (actualSpecial || expectedSpecial) {
            const ok = actualSpecial === expectedSpecial;
            return { ok, normalized: ok ? expectedRaw : "" };
        }
    }
    const entered = evaluateCExpression("0;", 0, actualRaw);
    if (entered.kind !== "ok" || !entered.result.valueLiteral) {
        return { ok: false, normalized: "" };
    }
    if ((targetKind === "floating" && entered.result.typeInfo.kind !== "floating") ||
        (targetKind === "integer" && entered.result.typeInfo.kind !== "integer")) {
        return { ok: false, normalized: "" };
    }
    // An enum's type is only defined in the program, but its values are ints.
    const castType = /^enum\b/.test(targetType) ? "int" : targetType;
    const cast = (raw) => evaluateCExpression("0;", 0, `(${castType})(${raw})`);
    const convertedActual = cast(actualRaw);
    const convertedExpected = cast(expectedRaw);
    if (convertedActual.kind !== "ok" || convertedExpected.kind !== "ok") {
        return { ok: false, normalized: "" };
    }
    const ok = convertedActual.result.value === convertedExpected.result.value &&
        convertedActual.result.exactValue === convertedExpected.result.exactValue;
    return { ok, normalized: ok ? convertedExpected.result.value : "" };
}
