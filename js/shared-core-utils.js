export function parseType(type = "int") {
    const clean = String(type || "").trim();
    const match = clean.match(/^(int|long|double)(\*+)?$/);
    if (!match)
        return { base: null, depth: 0 };
    const base = match[1];
    const depth = match[2] ? match[2].length : 0;
    return { base, depth };
}
export function isPointerType(type = "int") {
    const { depth } = parseType(type);
    return depth != null && Number.isFinite(depth) && depth > 0;
}
export function isRefCompatible(targetType = "int*", refType = "int") {
    const { base: targetBase, depth: targetDepth } = parseType(targetType);
    const { base: refBase, depth: refDepth } = parseType(refType);
    if (!targetBase ||
        !refBase ||
        targetDepth == null ||
        refDepth == null ||
        !Number.isFinite(targetDepth) ||
        !Number.isFinite(refDepth))
        return false;
    return targetBase === refBase && targetDepth === refDepth + 1;
}
export function getPointerDepth(type = "int") {
    const { depth } = parseType(type);
    return depth;
}
export function typeInfo(type = "int") {
    const { base, depth } = parseType(type);
    if (!base)
        return { size: 4, align: 4 };
    if (Number.isFinite(depth) && depth > 0)
        return { size: 8, align: 8 };
    if (base === "long" || base === "double")
        return { size: 8, align: 8 };
    return { size: 4, align: 4 };
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
    const trimmed = String(value ?? "").trim();
    if (trimmed === "-0")
        return "0";
    return trimmed;
}
function stripTrailingZeros(value) {
    const trimmed = String(value ?? "").trim();
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
    const text = String(value ?? "");
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
    const trimmed = String(value ?? "").trim();
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
    if (base === "double" && depth === 0) {
        const parsed = parseDoubleValueWithSign(value);
        if (parsed == null)
            return raw;
        const nanSign = opts.nanSign ?? parsed.nanSign;
        return formatDoubleStorage(parsed.value, nanSign);
    }
    return raw;
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
