export type BaseType =
  | "bool"
  | "char"
  | "signed char"
  | "unsigned char"
  | "short"
  | "unsigned short"
  | "int"
  | "unsigned int"
  | "long"
  | "unsigned long"
  | "long long"
  | "unsigned long long"
  | "float"
  | "double";
export interface ParsedType {
  base: BaseType | null;
  depth: number;
  arrayDims?: number[];
  pointeeArrayDims?: number[];
  pointeeInnerDepth?: number;
}
export interface TypeInfo {
  size: number;
  align: number;
}
export type BoxValue = string;
export interface BoxState {
  name: string;
  type: string;
  value: BoxValue;
  rawValue?: string | null;
  address?: string | null;
  arrayRoot?: string | null;
  arrayShape?: number[] | null;
  arrayIndices?: number[] | null;
  pointeeArrayDims?: number[] | null;
  pointeeInnerDepth?: number | null;
  names?: string[] | string | null;
  allowNameEdit?: boolean | null;
  allowTypeEdit?: boolean | null;
  allowDelete?: boolean | null;
  showDoubleExact?: boolean | null;
  node?: HTMLElement;
}

export type ProgramDiagnosticRange = {
  startLine: number;
  startCol: number;
  endLine: number;
  endCol: number;
};

export type ProgramDiagnostic = {
  kind: "compile" | "ub";
  message: string;
  file?: string;
  tip?: string;
  range: ProgramDiagnosticRange;
};
export type RandAddr = ((type?: string) => number) & {
  reset: (seed?: number | null) => void;
};

function normalizeTypeSpaces(value: string): string {
  return String(value || "").replace(/\s+/g, " ").trim();
}

export function canonicalizeBaseType(raw: string): BaseType | null {
  const base = normalizeTypeSpaces(raw);
  if (!base) return null;
  if (base === "_Bool" || base === "bool") return "bool";
  if (base === "char" || base === "signed char") return base;
  if (base === "unsigned char") return "unsigned char";
  if (
    base === "short" ||
    base === "short int" ||
    base === "signed short" ||
    base === "signed short int"
  )
    return "short";
  if (base === "unsigned short" || base === "unsigned short int")
    return "unsigned short";
  if (base === "int" || base === "signed" || base === "signed int")
    return "int";
  if (base === "unsigned" || base === "unsigned int") return "unsigned int";
  if (
    base === "long" ||
    base === "long int" ||
    base === "signed long" ||
    base === "signed long int"
  )
    return "long";
  if (base === "unsigned long" || base === "unsigned long int")
    return "unsigned long";
  if (
    base === "long long" ||
    base === "long long int" ||
    base === "signed long long" ||
    base === "signed long long int"
  )
    return "long long";
  if (
    base === "unsigned long long" ||
    base === "unsigned long long int"
  )
    return "unsigned long long";
  if (base === "float") return "float";
  if (base === "double") return "double";
  return null;
}

function parseBaseAndTrailingPointers(raw: string): {
  base: BaseType | null;
  pointerDepth: number;
} {
  const clean = String(raw || "").trim();
  if (!clean) return { base: null, pointerDepth: 0 };
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

export function parseType(type: string = "int"): ParsedType {
  const clean = String(type || "").trim();
  if (!clean) return { base: null, depth: 0 };
  const ptrArrayMatch = /^(.+?)\(\s*(\*+)\s*\)\s*((?:\[\s*\d+\s*\]\s*)+)\s*$/.exec(clean);
  if (ptrArrayMatch) {
    const leftSide = parseBaseAndTrailingPointers(ptrArrayMatch[1] || "");
    const base = leftSide.base;
    if (!base) return { base: null, depth: 0 };
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
  const arrayDims: number[] = [];
  while (true) {
    const m = /^(.*)\[\s*(\d+)\s*\]\s*$/.exec(remainder);
    if (!m) break;
    arrayDims.unshift(Number(m[2]));
    remainder = (m[1] || "").trim();
  }
  const parsedBase = parseBaseAndTrailingPointers(remainder);
  const base = parsedBase.base;
  const depth = parsedBase.pointerDepth;
  if (!base) return { base: null, depth: 0 };
  const parsed: ParsedType = { base, depth };
  if (arrayDims.length) parsed.arrayDims = arrayDims;
  return parsed;
}

export function isPointerType(type: string = "int"): boolean {
  const { depth } = parseType(type);
  return depth > 0;
}

export function isRefCompatible(
  targetType: string = "int*",
  refType: string = "int",
): boolean {
  const { base: targetBase, depth: targetDepth } = parseType(targetType);
  const { base: refBase, depth: refDepth } = parseType(refType);
  if (!targetBase || !refBase) return false;
  return targetBase === refBase && targetDepth === refDepth + 1;
}

export function getPointerDepth(type: string = "int"): number {
  const { depth } = parseType(type);
  return depth;
}

export function typeInfo(type: string = "int"): {
  size: number;
  align: number;
} {
  const { base, depth, arrayDims } = parseType(type);
  if (!base) return { size: 4, align: 4 };
  if (depth > 0) return { size: 8, align: 8 };
  const scalar = (() => {
    if (
      base === "long" ||
      base === "unsigned long" ||
      base === "long long" ||
      base === "unsigned long long" ||
      base === "double"
    ) {
      return { size: 8, align: 8 };
    }
    if (base === "short" || base === "unsigned short") {
      return { size: 2, align: 2 };
    }
    if (
      base === "bool" ||
      base === "char" ||
      base === "signed char" ||
      base === "unsigned char"
    ) {
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

let nextAddr: number | null = null;
export const randAddr: RandAddr = (type = "int") => {
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
randAddr.reset = function (seed: number | null = null) {
  nextAddr = seed;
};

export function normalizeZeroDisplay(value: BoxValue): string {
  const trimmed = value.trim();
  if (trimmed === "-0") return "0";
  return trimmed;
}

function stripTrailingZeros(value: string): string {
  const trimmed = value.trim();
  if (!trimmed.includes(".")) return trimmed === "-0" ? "0" : trimmed;
  let out = trimmed.replace(/0+$/, "");
  if (out.endsWith(".")) out = out.slice(0, -1);
  if (out === "-0") out = "0";
  return out;
}

function ensureDoubleDecimal(value: string): string {
  const text = value;
  return text.includes(".") ? text : `${text}.0`;
}

export function formatDoubleDefault(value: number, nanSign?: -1 | 1): string {
  const special = formatSpecialDouble(value, nanSign);
  if (special) return special;
  if (Object.is(value, -0)) return "-0.0";
  return ensureDoubleDecimal(stripTrailingZeros(value.toFixed(6)));
}

export function formatDoubleExact(value: number, nanSign?: -1 | 1): string {
  const special = formatSpecialDouble(value, nanSign);
  if (special) return special;
  if (Object.is(value, -0)) return "-0.0";
  const buf = new ArrayBuffer(8);
  const view = new DataView(buf);
  view.setFloat64(0, value, false);
  const bits = view.getBigUint64(0, false);
  const sign = bits >> 63n;
  const expBits = Number((bits >> 52n) & 0x7ffn);
  const frac = bits & ((1n << 52n) - 1n);
  if (expBits === 0x7ff) return String(value);
  const exponent = expBits === 0 ? -1022 : expBits - 1023;
  const significand = expBits === 0 ? frac : (1n << 52n) | frac;
  const exp2 = exponent - 52;
  let out = "";
  if (expBits === 0 && frac === 0n) {
    out = "0";
  } else if (exp2 >= 0) {
    out = (significand << BigInt(exp2)).toString();
  } else {
    const k = -exp2;
    const scaled = significand * 5n ** BigInt(k);
    let digits = scaled.toString();
    if (digits.length <= k) digits = digits.padStart(k + 1, "0");
    const intPart = digits.slice(0, digits.length - k);
    let fracPart = digits.slice(digits.length - k);
    fracPart = fracPart.replace(/0+$/, "");
    out = fracPart ? `${intPart}.${fracPart}` : intPart;
  }
  if (sign) out = `-${out}`;
  return ensureDoubleDecimal(out);
}

export function formatDoubleStorage(value: number, nanSign?: -1 | 1): string {
  const special = formatSpecialDouble(value, nanSign);
  if (special) return special;
  if (Object.is(value, -0)) return "-0.0";
  const text = String(value);
  if (/[eE]/.test(text)) return text;
  return text.includes(".") ? text : `${text}.0`;
}

export function doubleDisplayIsExact(
  defaultText: string,
  exactText: string,
): boolean {
  return defaultText === exactText;
}

export function normalizeSpecialFloatLiteral(value: string): string | null {
  const trimmed = value.trim();
  if (!trimmed) return null;
  const lower = trimmed.toLowerCase();
  if (lower === "nan") return "NaN";
  if (lower === "inf" || lower === "infinity") return "Infinity";
  return null;
}

export function formatSpecialDouble(
  value: number,
  nanSign?: -1 | 1,
): string | null {
  if (Number.isNaN(value)) return nanSign === -1 ? "-nan" : "nan";
  if (value === Infinity) return "inf";
  if (value === -Infinity) return "-inf";
  return null;
}

export function parseDoubleValueWithSign(
  value: BoxValue | bigint | number,
): { value: number; nanSign?: -1 | 1 } | null {
  if (typeof value === "number") {
    if (Number.isNaN(value)) return { value, nanSign: 1 };
    return { value };
  }
  if (typeof value === "bigint") {
    return { value: Number(value) };
  }
  const raw = String(value ?? "").trim();
  if (!raw) return null;
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

export function parseDoubleValue(
  value: BoxValue | bigint | number,
): number | null {
  const parsed = parseDoubleValueWithSign(value);
  return parsed ? parsed.value : null;
}

export function formatValueForType(
  value: BoxValue | bigint | number,
  type: string,
  opts: { nanSign?: -1 | 1 } = {},
): string {
  if (value === null || value === undefined) return "";
  const raw = String(value ?? "");
  if (raw === "") return "";
  const { base, depth } = parseType(type || "int");
  if ((base === "float" || base === "double") && depth === 0) {
    const parsed = parseDoubleValueWithSign(value);
    if (parsed == null) return raw;
    const nanSign = opts.nanSign ?? parsed.nanSign;
    return formatDoubleStorage(parsed.value, nanSign);
  }
  return raw;
}

export type ValueLiteralKind = "integer" | "floating";

export type ValueLiteralResult =
  | { kind: "empty"; trimmed: "" }
  | { kind: "invalid"; trimmed: string }
  | {
      kind: "ok";
      trimmed: string;
      type: string;
      value: BoxValue | bigint | number;
      literalKind: ValueLiteralKind;
      hasSuffix: boolean;
      nanSign?: -1 | 1;
    };

export type ValueLiteralEvaluation =
  | {
      result: {
        kind: string;
        type: string;
        value: BoxValue | bigint | number;
        valueLiteral: {
          kind: ValueLiteralKind;
          hasSuffix: boolean;
        };
        nanSign?: -1 | 1;
      };
    }
  | { error: unknown; kind: "compile" | "ub" };

export type ValueLiteralEvaluator = (literal: string) => ValueLiteralEvaluation;

export function parseValueLiteralInput(
  evaluator: ValueLiteralEvaluator,
  raw: BoxValue,
): ValueLiteralResult {
  const trimmed = raw.trim();
  if (!trimmed) return { kind: "empty", trimmed: "" };
  const evaluated = evaluator(trimmed);
  if ("error" in evaluated) return { kind: "invalid", trimmed };
  const result = evaluated.result;
  if (!result || result.kind !== "rvalue")
    return { kind: "invalid", trimmed };
  const literal = result.valueLiteral;
  if (!literal || (literal.kind !== "integer" && literal.kind !== "floating")) {
    return { kind: "invalid", trimmed };
  }
  const type = String(result.type || "").trim();
  if (!type) return { kind: "invalid", trimmed };
  return {
    kind: "ok",
    trimmed,
    type,
    value: result.value,
    literalKind: literal.kind,
    hasSuffix: literal.hasSuffix === true,
    nanSign: result.nanSign,
  };
}

export function valueLiteralMatchesTarget(
  value: ValueLiteralResult,
  targetType: string,
): boolean {
  if (value.kind !== "ok") return false;
  const target = parseType(targetType || "int");
  const literalType = parseType(value.type || "int");
  if (!target.base || !literalType.base || target.arrayDims?.length) return false;
  if (target.depth > 0) {
    return value.literalKind === "integer" && !value.hasSuffix;
  }
  if (value.literalKind === "integer") {
    if (target.base === "float" || target.base === "double") return false;
    return !value.hasSuffix || literalType.base === target.base;
  }
  if (target.base === "float") {
    return literalType.base === "float" || literalType.base === "double";
  }
  return target.base === "double" && literalType.base === "double";
}

function doubleValuesEqual(
  actualValue: number,
  expectedValue: number,
  actualNanSign?: -1 | 1,
  expectedNanSign?: -1 | 1,
): boolean {
  if (Number.isNaN(actualValue) || Number.isNaN(expectedValue)) {
    if (!Number.isNaN(actualValue) || !Number.isNaN(expectedValue)) return false;
    return (actualNanSign ?? 1) === (expectedNanSign ?? 1);
  }
  return Object.is(actualValue, expectedValue);
}

export function parsedLiteralValuesEqual(
  actual: ValueLiteralResult,
  expected: ValueLiteralResult,
  targetType: string,
): boolean {
  if (actual.kind !== "ok" || expected.kind !== "ok") return false;
  const target = parseType(targetType);
  if (target.depth === 0 && target.base === "float") {
    return doubleValuesEqual(
      Math.fround(Number(actual.value)),
      Math.fround(Number(expected.value)),
      actual.nanSign,
      expected.nanSign,
    );
  }
  if (target.depth === 0 && target.base === "double") {
    return doubleValuesEqual(
      Number(actual.value),
      Number(expected.value),
      actual.nanSign,
      expected.nanSign,
    );
  }
  try {
    return BigInt(actual.value as bigint) === BigInt(expected.value as bigint);
  } catch {
    return false;
  }
}

export function normalizeBoxValueForContext(
  evaluator: ValueLiteralEvaluator,
  box: BoxState,
): BoxState {
  const raw = box.rawValue ?? box.value ?? "";
  const parsed = parseValueLiteralInput(evaluator, raw);
  if (parsed.kind !== "ok") {
    return { ...box, value: parsed.trimmed };
  }
  const value = formatValueForType(parsed.value, parsed.type, {
    nanSign: parsed.nanSign,
  });
  return { ...box, value };
}

export function boxValueMatchesSpec(
  evaluator: ValueLiteralEvaluator,
  actual: BoxState,
  expected: BoxState,
): { ok: boolean; normalized: string } {
  const targetType = String(expected.type || actual.type || "").trim();
  const actualRaw = actual.rawValue ?? actual.value ?? "";
  const expectedRaw = expected.value ?? "";
  const actualTrimmed = (actualRaw ?? "").trim();
  const expectedTrimmed = (expectedRaw ?? "").trim();
  if (actualTrimmed === "" || expectedTrimmed === "") {
    const ok = actualTrimmed === "" && expectedTrimmed === "";
    return { ok, normalized: "" };
  }
  const parsedActual = parseValueLiteralInput(evaluator, actualRaw);
  const parsedExpected = parseValueLiteralInput(evaluator, expectedRaw);
  if (parsedActual.kind !== "ok" || parsedExpected.kind !== "ok") {
    return { ok: false, normalized: "" };
  }
  if (!valueLiteralMatchesTarget(parsedActual, targetType)) {
    return { ok: false, normalized: "" };
  }
  const ok = parsedLiteralValuesEqual(parsedActual, parsedExpected, targetType);
  if (!ok) return { ok: false, normalized: "" };
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
    if (ch === "/" && next === "/") break;
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
      while (i < src.length && src[i] !== "\n" && src[i] !== "\r") i++;
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

export type TextTokenReplacement = readonly [string, string];

export function replaceTextTokens(
  text: string,
  replacements: ReadonlyArray<TextTokenReplacement>,
): string {
  let out = String(text);
  for (const [needle, replacement] of replacements) {
    if (!needle) continue;
    out = out.split(needle).join(replacement);
  }
  return out;
}

export function applyTextTokenReplacements(
  value: string | string[] | null | undefined,
  replacements: ReadonlyArray<TextTokenReplacement>,
): string | string[] | null | undefined {
  if (value == null) return value;
  if (typeof value === "string") {
    return replaceTextTokens(value, replacements);
  }
  return value.map((part) => replaceTextTokens(part, replacements));
}

export function cloneBoxes(list: BoxState[] | null | undefined): BoxState[] {
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
