function normalizeTypeSpaces(value) {
    return String(value || "").replace(/\s+/g, " ").trim();
}
export function canonicalizeBaseType(raw) {
    const base = normalizeTypeSpaces(raw);
    if (!base)
        return null;
    if (base === "_Bool" || base === "bool")
        return "bool";
    if (base === "char" || base === "signed char")
        return base;
    if (base === "unsigned char")
        return "unsigned char";
    if (base === "short" ||
        base === "short int" ||
        base === "signed short" ||
        base === "signed short int")
        return "short";
    if (base === "unsigned short" || base === "unsigned short int")
        return "unsigned short";
    if (base === "int" || base === "signed" || base === "signed int")
        return "int";
    if (base === "unsigned" || base === "unsigned int")
        return "unsigned int";
    if (base === "long" ||
        base === "long int" ||
        base === "signed long" ||
        base === "signed long int")
        return "long";
    if (base === "unsigned long" || base === "unsigned long int")
        return "unsigned long";
    if (base === "long long" ||
        base === "long long int" ||
        base === "signed long long" ||
        base === "signed long long int")
        return "long long";
    if (base === "unsigned long long" ||
        base === "unsigned long long int")
        return "unsigned long long";
    if (base === "float")
        return "float";
    if (base === "double")
        return "double";
    return null;
}
function parseBaseAndTrailingPointers(raw) {
    const clean = String(raw || "").trim();
    if (!clean)
        return { base: null, pointerDepth: 0 };
    let idx = clean.length - 1;
    let pointerDepth = 0;
    while (idx >= 0) {
        const ch = clean[idx];
        if (ch === "*") {
            pointerDepth++;
            idx--;
            continue;
        }
        if (/\s/.test(ch)) {
            idx--;
            continue;
        }
        break;
    }
    return {
        base: canonicalizeBaseType(clean.slice(0, idx + 1)),
        pointerDepth,
    };
}
export function parseType(type = "int") {
    const clean = String(type || "").trim();
    if (!clean)
        return { base: null, depth: 0 };
    const ptrArrayMatch = /^(.+?)\(\s*(\*+)\s*\)\s*((?:\[\s*\d+\s*\]\s*)+)\s*$/.exec(clean);
    if (ptrArrayMatch) {
        const leftSide = parseBaseAndTrailingPointers(ptrArrayMatch[1] || "");
        const base = leftSide.base;
        if (!base)
            return { base: null, depth: 0 };
        const outerPointerDepth = String(ptrArrayMatch[2] || "").length;
        const dimsText = String(ptrArrayMatch[3] || "");
        const dims = [...dimsText.matchAll(/\[\s*(\d+)\s*\]/g)]
            .map((m) => Number(m[1]))
            .filter((n) => Number.isFinite(n) && n > 0);
        return {
            base,
            depth: leftSide.pointerDepth + outerPointerDepth,
            pointeeArrayDims: dims,
            pointeeInnerDepth: leftSide.pointerDepth,
        };
    }
    let remainder = clean;
    const arrayDims = [];
    while (true) {
        const m = /^(.*)\[\s*(\d+)\s*\]\s*$/.exec(remainder);
        if (!m)
            break;
        arrayDims.unshift(Number(m[2]));
        remainder = (m[1] || "").trim();
    }
    const parsedBase = parseBaseAndTrailingPointers(remainder);
    const base = parsedBase.base;
    const depth = parsedBase.pointerDepth;
    if (!base)
        return { base: null, depth: 0 };
    const parsed = { base, depth };
    if (arrayDims.length)
        parsed.arrayDims = arrayDims;
    return parsed;
}
export function isPointerType(type = "int") {
    const { depth } = parseType(type);
    return depth > 0;
}
export function isRefCompatible(targetType = "int*", refType = "int") {
    const { base: targetBase, depth: targetDepth } = parseType(targetType);
    const { base: refBase, depth: refDepth } = parseType(refType);
    if (!targetBase || !refBase)
        return false;
    return targetBase === refBase && targetDepth === refDepth + 1;
}
export function getPointerDepth(type = "int") {
    const { depth } = parseType(type);
    return depth;
}
export function typeInfo(type = "int") {
    const { base, depth, arrayDims } = parseType(type);
    if (!base)
        return { size: 4, align: 4 };
    if (depth > 0)
        return { size: 8, align: 8 };
    const scalar = (() => {
        if (base === "long" ||
            base === "unsigned long" ||
            base === "long long" ||
            base === "unsigned long long" ||
            base === "double") {
            return { size: 8, align: 8 };
        }
        if (base === "short" || base === "unsigned short") {
            return { size: 2, align: 2 };
        }
        if (base === "bool" ||
            base === "char" ||
            base === "signed char" ||
            base === "unsigned char") {
            return { size: 1, align: 1 };
        }
        return { size: 4, align: 4 };
    })();
    if (arrayDims?.length) {
        const count = arrayDims.reduce((acc, dim) => {
            const n = Math.max(0, Math.floor(Number(dim)));
            return acc * (n > 0 ? n : 0);
        }, 1);
        return { size: scalar.size * count, align: scalar.align };
    }
    return scalar;
}
let nextAddr = null;
export const randAddr = (type = "int") => {
    const { size, align } = typeInfo(type);
    if (nextAddr == null) {
        const base = 64 + Math.floor(Math.random() * 837);
        nextAddr = Math.max(align, Math.ceil(base / align) * align);
    }
    if (nextAddr % align !== 0) {
        nextAddr = Math.ceil(nextAddr / align) * align;
    }
    const addr = nextAddr;
    nextAddr = addr + size;
    return addr;
};
randAddr.reset = function (seed = null) {
    nextAddr = seed;
};
export function normalizeZeroDisplay(value) {
    const trimmed = value.trim();
    if (trimmed === "-0")
        return "0";
    return trimmed;
}
function stripTrailingZeros(value) {
    const trimmed = value.trim();
    if (!trimmed.includes("."))
        return trimmed === "-0" ? "0" : trimmed;
    let out = trimmed.replace(/0+$/, "");
    if (out.endsWith("."))
        out = out.slice(0, -1);
    if (out === "-0")
        out = "0";
    return out;
}
function ensureDoubleDecimal(value) {
    const text = value;
    return text.includes(".") ? text : `${text}.0`;
}
export function formatDoubleDefault(value, nanSign) {
    const special = formatSpecialDouble(value, nanSign);
    if (special)
        return special;
    if (Object.is(value, -0))
        return "-0.0";
    return ensureDoubleDecimal(stripTrailingZeros(value.toFixed(6)));
}
export function formatDoubleExact(value, nanSign) {
    const special = formatSpecialDouble(value, nanSign);
    if (special)
        return special;
    if (Object.is(value, -0))
        return "-0.0";
    const buf = new ArrayBuffer(8);
    const view = new DataView(buf);
    view.setFloat64(0, value, false);
    const bits = view.getBigUint64(0, false);
    const sign = bits >> 63n;
    const expBits = Number((bits >> 52n) & 0x7ffn);
    const frac = bits & ((1n << 52n) - 1n);
    if (expBits === 0x7ff)
        return String(value);
    const exponent = expBits === 0 ? -1022 : expBits - 1023;
    const significand = expBits === 0 ? frac : (1n << 52n) | frac;
    const exp2 = exponent - 52;
    let out = "";
    if (expBits === 0 && frac === 0n) {
        out = "0";
    }
    else if (exp2 >= 0) {
        out = (significand << BigInt(exp2)).toString();
    }
    else {
        const k = -exp2;
        const scaled = significand * 5n ** BigInt(k);
        let digits = scaled.toString();
        if (digits.length <= k)
            digits = digits.padStart(k + 1, "0");
        const intPart = digits.slice(0, digits.length - k);
        let fracPart = digits.slice(digits.length - k);
        fracPart = fracPart.replace(/0+$/, "");
        out = fracPart ? `${intPart}.${fracPart}` : intPart;
    }
    if (sign)
        out = `-${out}`;
    return ensureDoubleDecimal(out);
}
export function formatDoubleStorage(value, nanSign) {
    const special = formatSpecialDouble(value, nanSign);
    if (special)
        return special;
    if (Object.is(value, -0))
        return "-0.0";
    const text = String(value);
    if (/[eE]/.test(text))
        return text;
    return text.includes(".") ? text : `${text}.0`;
}
export function doubleDisplayIsExact(defaultText, exactText) {
    return defaultText === exactText;
}
export function normalizeSpecialFloatLiteral(value) {
    const trimmed = value.trim();
    if (!trimmed)
        return null;
    const lower = trimmed.toLowerCase();
    if (lower === "nan")
        return "NaN";
    if (lower === "inf" || lower === "infinity")
        return "Infinity";
    return null;
}
export function formatSpecialDouble(value, nanSign) {
    if (Number.isNaN(value))
        return nanSign === -1 ? "-nan" : "nan";
    if (value === Infinity)
        return "inf";
    if (value === -Infinity)
        return "-inf";
    return null;
}
export function parseDoubleValueWithSign(value) {
    if (typeof value === "number") {
        if (Number.isNaN(value))
            return { value, nanSign: 1 };
        return { value };
    }
    if (typeof value === "bigint") {
        return { value: Number(value) };
    }
    const raw = String(value ?? "").trim();
    if (!raw)
        return null;
    const sign = raw.startsWith("-") || raw.startsWith("+") ? raw[0] : "";
    const core = sign ? raw.slice(1) : raw;
    const normalized = normalizeSpecialFloatLiteral(core);
    if (normalized) {
        const parsed = Number(`${sign}${normalized}`);
        if (normalized === "NaN") {
            return { value: parsed, nanSign: sign === "-" ? -1 : 1 };
        }
        return { value: parsed };
    }
    const parsed = Number(raw);
    return Number.isNaN(parsed) ? null : { value: parsed };
}
export function parseDoubleValue(value) {
    const parsed = parseDoubleValueWithSign(value);
    return parsed ? parsed.value : null;
}
export function formatValueForType(value, type, opts = {}) {
    if (value === null || value === undefined)
        return "";
    const raw = String(value ?? "");
    if (raw === "")
        return "";
    const { base, depth } = parseType(type || "int");
    if ((base === "float" || base === "double") && depth === 0) {
        const parsed = parseDoubleValueWithSign(value);
        if (parsed == null)
            return raw;
        const nanSign = opts.nanSign ?? parsed.nanSign;
        return formatDoubleStorage(parsed.value, nanSign);
    }
    return raw;
}
function isNumericLiteralTokens(tokens) {
    if (tokens.length === 1) {
        return tokens[0]?.type === "number";
    }
    if (tokens.length === 2) {
        return (tokens[0]?.type === "sym" &&
            tokens[0]?.value === "-" &&
            tokens[1]?.type === "number");
    }
    return false;
}
export function parseValueExpressionInput(evaluator, raw) {
    const trimmed = raw.trim();
    if (!trimmed)
        return { kind: "empty", trimmed: "" };
    const tokens = evaluator.tokenizeProgram(trimmed);
    if (!tokens.length)
        return { kind: "invalid", trimmed };
    if (tokens.some((t) => t.type === "unknown"))
        return { kind: "invalid", trimmed };
    if (!isNumericLiteralTokens(tokens))
        return { kind: "invalid", trimmed };
    const evaluated = evaluator.evaluateExpressionText(trimmed, []);
    if ("error" in evaluated)
        return { kind: "invalid", trimmed };
    const result = evaluated.result;
    if (!result || result.kind !== "rvalue")
        return { kind: "invalid", trimmed };
    const type = String(result.type || "").trim();
    if (!type)
        return { kind: "invalid", trimmed };
    return {
        kind: "ok",
        trimmed,
        type,
        value: result.value,
        nanSign: result.nanSign,
    };
}
export function valueTypeMatchesTarget(valueType, targetType) {
    const isIntegerScalarBase = (base) => {
        if (!base)
            return false;
        return base !== "float" && base !== "double";
    };
    const sameArrayDims = (left, right) => {
        const a = left ?? [];
        const b = right ?? [];
        if (a.length !== b.length)
            return false;
        for (let i = 0; i < a.length; i++) {
            if (a[i] !== b[i])
                return false;
        }
        return true;
    };
    const target = parseType(targetType || "int");
    const value = parseType(valueType || "int");
    if (!target.base || !value.base)
        return false;
    if (target.depth > 0 && value.depth === 0) {
        return isIntegerScalarBase(value.base);
    }
    if (target.depth > 0 || value.depth > 0) {
        return (target.base === value.base &&
            target.depth === value.depth &&
            sameArrayDims(target.pointeeArrayDims, value.pointeeArrayDims) &&
            (target.pointeeInnerDepth || 0) === (value.pointeeInnerDepth || 0));
    }
    return true;
}
function doubleValuesEqual(actualValue, expectedValue, actualNanSign, expectedNanSign) {
    if (Number.isNaN(actualValue) || Number.isNaN(expectedValue)) {
        if (!Number.isNaN(actualValue) || !Number.isNaN(expectedValue))
            return false;
        return (actualNanSign ?? 1) === (expectedNanSign ?? 1);
    }
    return Object.is(actualValue, expectedValue);
}
export function parsedValuesEqual(actual, expected) {
    if (actual.kind !== "ok" || expected.kind !== "ok")
        return false;
    const parsedExpectedType = parseType(expected.type);
    if (parsedExpectedType.depth === 0 &&
        (parsedExpectedType.base === "float" || parsedExpectedType.base === "double")) {
        return doubleValuesEqual(Number(actual.value), Number(expected.value), actual.nanSign, expected.nanSign);
    }
    if (parsedExpectedType.depth > 0) {
        try {
            return BigInt(actual.value) === BigInt(expected.value);
        }
        catch {
            return String(actual.value ?? "").trim() === String(expected.value ?? "").trim();
        }
    }
    try {
        return BigInt(actual.value) === BigInt(expected.value);
    }
    catch {
        return false;
    }
}
export function normalizeBoxValueForContext(evaluator, box) {
    const raw = box.rawValue ?? box.value ?? "";
    const parsed = parseValueExpressionInput(evaluator, raw);
    if (parsed.kind !== "ok") {
        return { ...box, value: parsed.trimmed };
    }
    const value = formatValueForType(parsed.value, parsed.type, {
        nanSign: parsed.nanSign,
    });
    return { ...box, value };
}
export function boxValueMatchesSpec(evaluator, actual, expected) {
    const targetType = String(expected.type || actual.type || "").trim();
    const actualRaw = actual.rawValue ?? actual.value ?? "";
    const expectedRaw = expected.value ?? "";
    const actualTrimmed = (actualRaw ?? "").trim();
    const expectedTrimmed = (expectedRaw ?? "").trim();
    if (actualTrimmed === "" || expectedTrimmed === "") {
        const ok = actualTrimmed === "" && expectedTrimmed === "";
        return { ok, normalized: "" };
    }
    const parsedActual = parseValueExpressionInput(evaluator, actualRaw);
    const parsedExpected = parseValueExpressionInput(evaluator, expectedRaw);
    if (parsedActual.kind !== "ok" || parsedExpected.kind !== "ok") {
        return { ok: false, normalized: "" };
    }
    if (!valueTypeMatchesTarget(parsedActual.type, targetType)) {
        return { ok: false, normalized: "" };
    }
    if (!valueTypeMatchesTarget(parsedExpected.type, targetType)) {
        return { ok: false, normalized: "" };
    }
    const ok = parsedValuesEqual(parsedActual, parsedExpected);
    if (!ok)
        return { ok: false, normalized: "" };
    const normalized = formatValueForType(parsedExpected.value, targetType, {
        nanSign: parsedExpected.nanSign,
    });
    return { ok: true, normalized };
}
export function stripLineComments(src = "") {
    let out = "";
    let i = 0;
    let inBlock = false;
    while (i < src.length) {
        const ch = src[i];
        const next = src[i + 1];
        if (inBlock) {
            if (ch === "*" && next === "/") {
                inBlock = false;
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }
        if (ch === "/" && next === "/")
            break;
        if (ch === "/" && next === "*") {
            inBlock = true;
            i += 2;
            continue;
        }
        out += ch;
        i += 1;
    }
    return { text: out, unterminated: inBlock };
}
export function stripAllComments(src = "") {
    let out = "";
    let i = 0;
    let inBlock = false;
    while (i < src.length) {
        const ch = src[i];
        const next = src[i + 1];
        if (inBlock) {
            if (ch === "*" && next === "/") {
                inBlock = false;
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }
        if (ch === "/" && next === "/") {
            i += 2;
            while (i < src.length && src[i] !== "\n" && src[i] !== "\r")
                i++;
            continue;
        }
        if (ch === "/" && next === "*") {
            inBlock = true;
            i += 2;
            continue;
        }
        out += ch;
        i += 1;
    }
    return out;
}
export function replaceTextTokens(text, replacements) {
    let out = String(text);
    for (const [needle, replacement] of replacements) {
        if (!needle)
            continue;
        out = out.split(needle).join(replacement);
    }
    return out;
}
export function applyTextTokenReplacements(value, replacements) {
    if (value == null)
        return value;
    if (typeof value === "string") {
        return replaceTextTokens(value, replacements);
    }
    return value.map((part) => replaceTextTokens(part, replacements));
}
export function cloneBoxes(list) {
    return Array.isArray(list)
        ? list.map((b) => ({
            ...b,
            names: Array.isArray(b.names)
                ? [...b.names]
                : b.names
                    ? [b.names]
                    : b.name
                        ? [b.name]
                        : [],
        }))
        : [];
}
