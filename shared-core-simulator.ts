import type { BoxState, BoxValue } from "./shared-core-utils.js";
import {
  canonicalizeBaseType,
  cloneBoxes,
  formatValueForType,
  normalizeSpecialFloatLiteral,
  parseDoubleValueWithSign,
  parseType,
  randAddr,
  stripAllComments,
  typeInfo,
} from "./shared-core-utils.js";
import {
  createParserTools,
  type BinaryOp,
  type DeclaredNames,
  type ExprNode,
  type Statement,
  type StatementMap,
  type StatementPart,
  type StatementRange,
  type Token,
} from "./shared-core-parser.js";

export type {
  UnaryOp,
  PostfixOp,
  BinaryOp,
  AssignmentOp,
  ExprNode,
  Statement,
  Token,
  StatementPart,
  StatementRange,
  StatementMap,
} from "./shared-core-parser.js";
export type StatementContext = {
  statementMap: StatementMap;
  currentRange: StatementRange | null;
  prevRange: StatementRange | null;
  midStatement: boolean;
  atStatementStart: boolean;
};
export type IfBlock = {
  headerIndex: number;
  headerStartLine: number;
  headerEndLine: number;
  openIndex: number;
  closeIndex: number;
  trueTarget: number;
  falseTarget: number;
  elseIndex?: number | null;
  elseOpenIndex?: number | null;
  elseCloseIndex?: number | null;
  elseTarget?: number | null;
  afterIndex: number;
  expr: ExprNode;
  hasVar: boolean;
};
export type WhileBlock = {
  headerIndex: number;
  headerStartLine: number;
  headerEndLine: number;
  openIndex: number;
  closeIndex: number;
  trueTarget: number;
  afterIndex: number;
  expr: ExprNode;
  hasVar: boolean;
};
export type IfBlockMap = {
  map: Map<number, IfBlock>;
};
export type WhileBlockMap = {
  map: Map<number, WhileBlock>;
};
export type ConditionResult =
  | { value: boolean }
  | { error: true; kind: "compile" | "ub" };
export type ProgramResult =
  | { kind: "ok"; state: BoxState[] }
  | { kind: "compile" | "ub" };
export type ProgramTrace = {
  state: BoxState[];
  nextIndex: number;
  executedSteps: number;
};
export type ProgramDiagnosticRange = {
  startLine: number;
  startCol: number;
  endLine: number;
  endCol: number;
};
export type ProgramDiagnostic = {
  kind: "compile" | "ub";
  message: string;
  tip?: string;
  range: ProgramDiagnosticRange;
};
export interface SimpleSimulator {
  tokenizeProgram: (src?: string) => Token[];
  splitStatements: (tokens: Token[]) => StatementPart[];
  parseStatements: (text: string) => Statement[];
  buildStatementMap: (lines: string[]) => StatementMap;
  buildIfStatementMap: (
    parts: StatementPart[],
    opts?: { lastLine?: number },
  ) => IfBlockMap;
  buildWhileStatementMap: (
    parts: StatementPart[],
    opts?: { lastLine?: number },
  ) => WhileBlockMap;
  statementRangeForLine: (
    statementMap: StatementMap,
    lineIndex: number,
  ) => StatementRange | null;
  getStatementContext: (lines: string[], boundary: number) => StatementContext;
  evaluateCondition: (expr: ExprNode, state: BoxState[]) => ConditionResult;
  evaluateExpressionText: (
    expr: string,
    state: BoxState[],
    opts?: { allowSideEffects?: boolean },
  ) =>
    | {
        result: {
          kind: string;
          type: string;
          value: BoxValue | bigint | number;
          address: string;
          nanSign?: -1 | 1;
        };
      }
    | {
        error: true;
        kind: "compile" | "ub";
      };
  findMissingSemicolonLines: (text: string) => number[];
  applyProgramParts: (
    parts: StatementPart[],
    opts?: { alloc?: (type?: string) => string; stop?: number },
  ) => BoxState[] | null;
  analyzeProgramParts: (
    parts: StatementPart[],
    opts?: { alloc?: (type?: string) => string; stop?: number },
  ) => ProgramResult;
  traceProgramParts: (
    parts: StatementPart[],
    opts?: { alloc?: (type?: string) => string; stopSteps?: number },
  ) => ProgramTrace | null;
  diagnoseProgram: (
    text: string,
    opts?: { alloc?: (type?: string) => string },
  ) => ProgramDiagnostic[];
  applyProgram: (
    text: string,
    opts?: { alloc?: (type?: string) => string },
  ) => BoxState[] | null;
}

export function createSimpleSimulator(): SimpleSimulator {
  const requireSourceValue = true;

  type ScopeStack = Array<Set<string>>;
  type EvalError = {
    error: true;
    kind: "compile" | "ub";
  };
  type EvalValue = {
    kind: "lvalue" | "rvalue";
    base: string;
    depth: number;
    value: BoxValue | bigint | number;
    address: string;
    label: string;
    nanSign?: -1 | 1;
    isArray?: boolean;
    arrayShape?: number[];
    pointeeArrayDims?: number[];
    pointeeInnerDepth?: number;
    error?: undefined;
  };
  type EvalResult = EvalValue | EvalError;
  type ScalarResult =
    | {
        value: bigint | number;
        base: string;
        nanSign?: -1 | 1;
        error?: undefined;
      }
    | EvalError;
  type StatementValidationResult =
    | {
        error: true;
        kind: "compile" | "ub";
      }
    | { parsed: Statement; next: BoxState[] };
  const isEvalError = (result: EvalResult): result is EvalError =>
    !!result.error;
  const isScalarError = (result: ScalarResult): result is EvalError =>
    !!result.error;
  const makeCompileError = (): EvalError => ({
    error: true,
    kind: "compile",
  });
  const makeUbError = (): EvalError => ({
    error: true,
    kind: "ub",
  });

  function decayArrayValue(result: EvalResult): EvalResult {
    if (isEvalError(result)) return result;
    if (!result.isArray) return result;
    const arrayShape = normalizeArrayDims(result.arrayShape);
    const decayedPointeeArrayDims =
      arrayShape.length > 1
        ? arrayShape.slice(1)
        : normalizeArrayDims(result.pointeeArrayDims);
    return {
      kind: "rvalue",
      base: result.base,
      depth: result.depth,
      value: result.value,
      address: "",
      label: result.label,
      nanSign: result.nanSign,
      pointeeArrayDims: decayedPointeeArrayDims,
      pointeeInnerDepth: normalizePointeeInnerDepth(
        result.pointeeInnerDepth,
        result.depth,
        decayedPointeeArrayDims,
      ),
    };
  }

  function arrayElementName(name: string, indices: number[]): string {
    return `${name}${indices.map((index) => `[${index}]`).join("")}`;
  }

  function parseArrayElementName(
    name: string,
  ): { baseName: string; indices: number[] } | null {
    const match = /^([A-Za-z_][A-Za-z0-9_]*)((?:\[\d+\])+)$/.exec(
      String(name || ""),
    );
    if (!match) return null;
    const baseName = match[1] || "";
    const suffix = match[2] || "";
    if (!baseName || !suffix) return null;
    const indices: number[] = [];
    const rx = /\[(\d+)\]/g;
    let m: RegExpExecArray | null;
    while ((m = rx.exec(suffix)) != null) {
      const value = Number(m[1]);
      if (!Number.isFinite(value) || value < 0) return null;
      indices.push(value);
    }
    if (!indices.length) return null;
    return { baseName, indices };
  }

  function forEachArrayIndex(
    shape: number[],
    fn: (indices: number[], linearIndex: number) => void,
  ): void {
    const dims = shape.map((d) => Math.max(0, Math.floor(Number(d))));
    if (!dims.length || dims.some((d) => d <= 0)) return;
    let linear = 0;
    const indices = new Array(dims.length).fill(0);
    const recur = (depth: number) => {
      if (depth >= dims.length) {
        fn(indices.slice(), linear++);
        return;
      }
      for (let i = 0; i < dims[depth]!; i++) {
        indices[depth] = i;
        recur(depth + 1);
      }
    };
    recur(0);
  }

  function normalizeArrayDims(dims: unknown): number[] {
    if (!Array.isArray(dims)) return [];
    const out: number[] = [];
    for (const dim of dims) {
      const value = Math.floor(Number(dim));
      if (!Number.isFinite(value) || value <= 0) return [];
      out.push(value);
    }
    return out;
  }

  function sameArrayDims(left: number[], right: number[]): boolean {
    if (left.length !== right.length) return false;
    for (let i = 0; i < left.length; i++) {
      if (left[i] !== right[i]) return false;
    }
    return true;
  }

  function normalizePointeeInnerDepth(
    value: unknown,
    depth: number,
    pointeeArrayDims: number[],
  ): number {
    if (!pointeeArrayDims.length) return 0;
    const raw = Math.floor(Number(value));
    const normalized = Number.isFinite(raw) ? raw : 0;
    const maxInner = Math.max(0, Math.floor(Number(depth)) - 1);
    return Math.max(0, Math.min(normalized, maxInner));
  }

  function samePointerPointeeType(
    leftDepth: number,
    leftPointeeArrayDims: number[],
    leftPointeeInnerDepth: unknown,
    rightDepth: number,
    rightPointeeArrayDims: number[],
    rightPointeeInnerDepth: unknown,
  ): boolean {
    if (!sameArrayDims(leftPointeeArrayDims, rightPointeeArrayDims)) return false;
    if (!leftPointeeArrayDims.length) return true;
    return (
      normalizePointeeInnerDepth(
        leftPointeeInnerDepth,
        leftDepth,
        leftPointeeArrayDims,
      ) ===
      normalizePointeeInnerDepth(
        rightPointeeInnerDepth,
        rightDepth,
        rightPointeeArrayDims,
      )
    );
  }

  function arrayElementCount(shape: number[]): number {
    let count = 1;
    for (const dim of shape) {
      count *= dim;
    }
    return count;
  }

  function arrayLinearIndex(indices: number[], shape: number[]): number | null {
    if (indices.length !== shape.length) return null;
    let linear = 0;
    let stride = 1;
    for (let i = shape.length - 1; i >= 0; i--) {
      const idx = indices[i]!;
      const dim = shape[i]!;
      if (idx < 0 || idx >= dim) return null;
      linear += idx * stride;
      stride *= dim;
    }
    return linear;
  }

  function addDeclaredNames(
    scopes: ScopeStack,
    declared: DeclaredNames,
    names: string[],
  ) {
    for (const name of names) declared.add(name);
    const current = scopes[scopes.length - 1];
    if (current) {
      for (const name of names) current.add(name);
    }
  }

  function popScope(
    scopes: ScopeStack,
    declared: DeclaredNames,
    state: BoxState[],
  ): { state: BoxState[]; error?: true } {
    if (scopes.length <= 1) {
      return { state, error: true };
    }
    const frame = scopes.pop();
    if (!frame || frame.size === 0) return { state };
    const namesToRemove = new Set(frame);
    const nextState = state.filter((box) => !namesToRemove.has(box.name));
    frame.forEach((name) => declared.delete(name));
    return { state: nextState };
  }

  function makePointerType(
    depth: number,
    base: string = "int",
    pointeeArrayDims: number[] = [],
    pointeeInnerDepth?: number,
  ): string | null {
    if (depth < 0) return null;
    const canonicalBase = canonicalizeBaseType(base);
    if (!canonicalBase) return null;
    const dims = normalizeArrayDims(pointeeArrayDims)
      .map((d) => `[${d}]`)
      .join("");
    if (depth === 0) return `${canonicalBase}${dims}`;
    if (pointeeArrayDims.length) {
      const rawInner = Math.floor(Number(pointeeInnerDepth));
      const innerDepth = Math.max(
        0,
        Math.min(
          Number.isFinite(rawInner) ? rawInner : 0,
          Math.max(0, Math.floor(depth)),
        ),
      );
      const outerDepth = Math.max(0, depth - innerDepth);
      const inner = innerDepth > 0 ? ` ${"*".repeat(innerDepth)}` : "";
      if (outerDepth === 0) {
        return `${canonicalBase}${inner}${dims}`;
      }
      return `${canonicalBase}${inner} (${ "*".repeat(outerDepth) })${dims}`;
    }
    return `${canonicalBase}${"*".repeat(depth)}`;
  }

  type IntegerMeta = {
    bits: number;
    signed: boolean;
    rank: number;
  };

  const INTEGER_TYPE_META: Record<string, IntegerMeta> = {
    bool: { bits: 1, signed: false, rank: 1 },
    char: { bits: 8, signed: true, rank: 2 },
    "signed char": { bits: 8, signed: true, rank: 2 },
    "unsigned char": { bits: 8, signed: false, rank: 2 },
    short: { bits: 16, signed: true, rank: 3 },
    "unsigned short": { bits: 16, signed: false, rank: 3 },
    int: { bits: 32, signed: true, rank: 4 },
    "unsigned int": { bits: 32, signed: false, rank: 4 },
    long: { bits: 64, signed: true, rank: 5 },
    "unsigned long": { bits: 64, signed: false, rank: 5 },
    "long long": { bits: 64, signed: true, rank: 6 },
    "unsigned long long": { bits: 64, signed: false, rank: 6 },
  };

  function integerMetaForBase(base: string): IntegerMeta | null {
    return INTEGER_TYPE_META[base] || null;
  }

  function isFloatingBase(base: string): boolean {
    return base === "float" || base === "double";
  }

  function isIntegerBase(base: string): boolean {
    return !!integerMetaForBase(base);
  }

  function integerRangeForBase(
    base: string,
  ): { min: bigint; max: bigint; bits: number; signed: boolean } | null {
    const meta = integerMetaForBase(base);
    if (!meta) return null;
    if (!meta.signed) {
      return {
        min: 0n,
        max: (1n << BigInt(meta.bits)) - 1n,
        bits: meta.bits,
        signed: false,
      };
    }
    const max = (1n << BigInt(meta.bits - 1)) - 1n;
    const min = -(1n << BigInt(meta.bits - 1));
    return { min, max, bits: meta.bits, signed: true };
  }

  function stripIntegerSuffix(value: string): string {
    const raw = value.trim();
    if (!raw) return raw;
    const parsed = parseIntegerSuffixInfo(raw);
    if (!parsed) return raw;
    return parsed.core;
  }

  function parseIntegerSuffixInfo(
    value: string,
  ): { core: string; unsigned: boolean; longCount: 0 | 1 | 2 } | null {
    const raw = String(value || "").trim();
    if (!raw) return null;
    let idx = raw.length;
    while (idx > 0 && /[uUlL]/.test(raw[idx - 1]!)) idx--;
    const core = raw.slice(0, idx);
    const suffix = raw.slice(idx).toLowerCase();
    if (!suffix) {
      return { core, unsigned: false, longCount: 0 };
    }
    if (
      suffix !== "u" &&
      suffix !== "l" &&
      suffix !== "ll" &&
      suffix !== "ul" &&
      suffix !== "lu" &&
      suffix !== "ull" &&
      suffix !== "llu"
    ) {
      return null;
    }
    const unsigned = suffix.includes("u");
    const longCount = suffix.includes("ll")
      ? 2
      : suffix.includes("l")
        ? 1
        : 0;
    return { core, unsigned, longCount };
  }

  function parseIntegerLiteral(value: string): bigint | null {
    const raw = stripIntegerSuffix(value);
    if (!raw) return null;
    if (raw.startsWith("0x") || raw.startsWith("0X")) {
      const digits = raw.slice(2);
      if (!digits || !/^[0-9a-fA-F]+$/.test(digits)) return null;
      try {
        return BigInt(`0x${digits}`);
      } catch {
        return null;
      }
    }
    if (raw.length > 1 && raw.startsWith("0")) {
      if (!/^0[0-7]+$/.test(raw)) return null;
      try {
        return BigInt(`0o${raw.slice(1)}`);
      } catch {
        return null;
      }
    }
    try {
      return BigInt(raw);
    } catch {
      return null;
    }
  }

  function isSpecialFloatLiteral(value: string): boolean {
    return !!normalizeSpecialFloatLiteral(value);
  }

  function isDecimalLiteral(value: string): boolean {
    return /[.eE]/.test(String(value));
  }

  function isNonDecimalIntegerLiteral(value: string): boolean {
    const core = stripIntegerSuffix(value);
    return /^0[xX]/.test(core) || /^0[0-7]/.test(core);
  }

  function fitsIntegerLiteralInBase(value: bigint, base: string): boolean {
    const range = integerRangeForBase(base);
    if (!range) return false;
    return value >= range.min && value <= range.max;
  }

  function inferIntegerLiteralBase(value: string): string | null {
    const info = parseIntegerSuffixInfo(value);
    if (!info) return null;
    const literalValue = parseIntegerLiteral(value);
    if (literalValue == null) return null;
    const nonDecimal = isNonDecimalIntegerLiteral(value);
    const candidates: string[] = [];
    if (info.unsigned) {
      if (info.longCount === 0) candidates.push("unsigned int", "unsigned long", "unsigned long long");
      else if (info.longCount === 1) candidates.push("unsigned long", "unsigned long long");
      else candidates.push("unsigned long long");
    } else if (info.longCount === 0) {
      if (nonDecimal) {
        candidates.push(
          "int",
          "unsigned int",
          "long",
          "unsigned long",
          "long long",
          "unsigned long long",
        );
      } else {
        candidates.push("int", "long", "long long");
      }
    } else if (info.longCount === 1) {
      if (nonDecimal) {
        candidates.push("long", "unsigned long", "long long", "unsigned long long");
      } else {
        candidates.push("long", "long long");
      }
    } else {
      if (nonDecimal) {
        candidates.push("long long", "unsigned long long");
      } else {
        candidates.push("long long");
      }
    }
    for (const candidate of candidates) {
      if (fitsIntegerLiteralInBase(literalValue, candidate)) return candidate;
    }
    return null;
  }

  function parseNumericLiteralValue(
    value: string,
  ):
    | { base: string; value: bigint | number; nanSign?: -1 | 1 }
    | EvalError {
    const trimmed = String(value || "").trim();
    if (!trimmed) {
      return makeCompileError();
    }
    if (isSpecialFloatLiteral(trimmed)) {
      const parsed = parseDoubleValueWithSign(trimmed);
      if (!parsed) {
        return makeCompileError();
      }
      return { base: "double", value: parsed.value, nanSign: parsed.nanSign };
    }
    if (isDecimalLiteral(trimmed)) {
      const lower = trimmed.toLowerCase();
      if (lower.endsWith("l")) {
        return makeCompileError();
      }
      const isFloat = lower.endsWith("f");
      const core = isFloat ? trimmed.slice(0, -1) : trimmed;
      const parsed = parseDoubleValueWithSign(core);
      if (!parsed || !Number.isFinite(parsed.value) && !Number.isNaN(parsed.value)) {
        return makeCompileError();
      }
      const base = isFloat ? "float" : "double";
      const castValue = base === "float" ? Math.fround(parsed.value) : parsed.value;
      return { base, value: castValue, nanSign: parsed.nanSign };
    }
    const literalBase = inferIntegerLiteralBase(trimmed);
    if (!literalBase) {
      return makeCompileError();
    }
    const parsedInt = parseIntegerLiteral(trimmed);
    if (parsedInt == null) {
      return makeCompileError();
    }
    const wrapped = normalizeIntegerForBase(parsedInt, literalBase);
    if (wrapped == null) {
      return makeCompileError();
    }
    return { base: literalBase, value: wrapped };
  }

  function integerOverflowError(_base: string): EvalError {
    return makeUbError();
  }

  function checkIntegerRange(value: bigint, base: string): EvalError | null {
    const range = integerRangeForBase(base);
    if (!range) return null;
    if (value < range.min || value > range.max)
      return integerOverflowError(base);
    return null;
  }

  function bitWidthForBase(base: string): number | null {
    const meta = integerMetaForBase(base);
    return meta ? meta.bits : null;
  }

  function unsignedVariantForBase(base: string): string | null {
    if (base === "char" || base === "signed char" || base === "unsigned char")
      return "unsigned char";
    if (base === "short" || base === "unsigned short") return "unsigned short";
    if (base === "int" || base === "unsigned int") return "unsigned int";
    if (base === "long" || base === "unsigned long") return "unsigned long";
    if (base === "long long" || base === "unsigned long long")
      return "unsigned long long";
    if (base === "bool") return "bool";
    return null;
  }

  function integerPromotionBase(base: string): string {
    const meta = integerMetaForBase(base);
    if (!meta) return "int";
    if (meta.rank < INTEGER_TYPE_META.int.rank) return "int";
    return base;
  }

  function canSignedRepresentUnsigned(
    signedBase: string,
    unsignedBase: string,
  ): boolean {
    const signedRange = integerRangeForBase(signedBase);
    const unsignedRange = integerRangeForBase(unsignedBase);
    if (!signedRange || !unsignedRange) return false;
    if (!signedRange.signed || unsignedRange.signed) return false;
    return signedRange.max >= unsignedRange.max;
  }

  function usualIntegerBase(leftBase: string, rightBase: string): string {
    const leftPromoted = integerPromotionBase(leftBase);
    const rightPromoted = integerPromotionBase(rightBase);
    const leftMeta = integerMetaForBase(leftPromoted);
    const rightMeta = integerMetaForBase(rightPromoted);
    if (!leftMeta || !rightMeta) return "int";
    if (leftPromoted === rightPromoted) return leftPromoted;
    if (leftMeta.signed === rightMeta.signed) {
      return leftMeta.rank >= rightMeta.rank ? leftPromoted : rightPromoted;
    }
    const signedBase = leftMeta.signed ? leftPromoted : rightPromoted;
    const unsignedBase = leftMeta.signed ? rightPromoted : leftPromoted;
    const signedMeta = integerMetaForBase(signedBase);
    const unsignedMeta = integerMetaForBase(unsignedBase);
    if (!signedMeta || !unsignedMeta) return "int";
    if (unsignedMeta.rank >= signedMeta.rank) {
      return unsignedBase;
    }
    if (canSignedRepresentUnsigned(signedBase, unsignedBase)) {
      return signedBase;
    }
    return unsignedVariantForBase(signedBase) || unsignedBase;
  }

  function wrapIntegerToBase(value: bigint, base: string): bigint | null {
    const range = integerRangeForBase(base);
    if (!range) return null;
    const width = range.bits;
    const modulo = 1n << BigInt(width);
    let wrapped = value % modulo;
    if (wrapped < 0n) wrapped += modulo;
    if (range.signed) {
      const signBit = 1n << BigInt(width - 1);
      if (wrapped >= signBit) wrapped -= modulo;
    }
    return wrapped;
  }

  function normalizeIntegerForBase(value: bigint, base: string): bigint | null {
    return wrapIntegerToBase(value, base);
  }

  const parser = createParserTools({
    evaluateArrayLengthExpr: (expr: ExprNode): number | null => {
      const evaluated = evaluateExpression(expr, [], {
        requireValue: true,
      });
      if (isScalarError(evaluated)) return null;
      if (!isIntegerBase(evaluated.base)) return null;
      const raw = evaluated.value;
      if (typeof raw !== "bigint") return null;
      if (raw <= 0n) return null;
      if (raw > BigInt(Number.MAX_SAFE_INTEGER)) return null;
      return Number(raw);
    },
  });
  const tokenizeProgram = parser.tokenizeProgram;
  const parseExpressionTokens = parser.parseExpressionTokens;
  const parseDeclHead = parser.parseDeclHead;
  const parseIfHeaderTokens = parser.parseIfHeaderTokens;
  const parseWhileHeaderTokens = parser.parseWhileHeaderTokens;
  const parseStatementTokens = parser.parseStatementTokens;
  const controlHeaderEndIndex = parser.controlHeaderEndIndex;
  const splitStatements = parser.splitStatements;

  const DIAGNOSTIC_TYPE_KEYWORDS = new Set([
    "signed",
    "unsigned",
    "short",
    "long",
    "int",
    "char",
    "float",
    "double",
    "_Bool",
    "bool",
  ]);
  const DIAGNOSTIC_BINARY_OPERATORS = new Set([
    "=",
    "+=",
    "-=",
    "*=",
    "/=",
    "%=",
    "<<=",
    ">>=",
    "&=",
    "^=",
    "|=",
    "+",
    "-",
    "*",
    "/",
    "%",
    "<<",
    ">>",
    "<",
    "<=",
    ">",
    ">=",
    "==",
    "!=",
    "&&",
    "||",
    "&",
    "^",
    "|",
  ]);
  const DIAGNOSTIC_PREFIX_OPERATORS = new Set([
    "+",
    "-",
    "!",
    "~",
    "*",
    "&",
    "++",
    "--",
  ]);

  function tokenWidth(token: Token): number {
    return Math.max(1, String(token.value || "").length);
  }

  function lineLength(lines: string[], lineIndex: number): number {
    const line = lines[Math.max(0, Math.min(lines.length - 1, lineIndex))] || "";
    return line.length;
  }

  function normalizeDiagnosticRange(
    range: ProgramDiagnosticRange,
    lines: string[],
  ): ProgramDiagnosticRange {
    if (!lines.length) {
      return {
        startLine: 0,
        startCol: 0,
        endLine: 0,
        endCol: 1,
      };
    }
    const startLine = Math.max(0, Math.min(lines.length - 1, range.startLine));
    const endLine = Math.max(startLine, Math.min(lines.length - 1, range.endLine));
    const maxStartCol = lineLength(lines, startLine);
    const maxEndCol = lineLength(lines, endLine);
    let startCol = Math.max(0, Math.min(maxStartCol, range.startCol));
    let endCol = Math.max(0, Math.min(maxEndCol, range.endCol));
    if (startLine === endLine && endCol <= startCol) {
      if (maxEndCol > 0) {
        startCol = Math.max(0, Math.min(startCol, maxEndCol - 1));
        endCol = Math.min(maxEndCol, startCol + 1);
      } else {
        startCol = 0;
        endCol = 1;
      }
    }
    return {
      startLine,
      startCol,
      endLine,
      endCol,
    };
  }

  function rangeFromToken(token: Token, lines: string[]): ProgramDiagnosticRange {
    return normalizeDiagnosticRange(
      {
        startLine: token.line,
        startCol: token.col,
        endLine: token.line,
        endCol: token.col + tokenWidth(token),
      },
      lines,
    );
  }

  function rangeFromTokens(
    tokens: Token[],
    lines: string[],
  ): ProgramDiagnosticRange {
    if (!tokens.length) {
      return normalizeDiagnosticRange(
        {
          startLine: 0,
          startCol: 0,
          endLine: 0,
          endCol: 1,
        },
        lines,
      );
    }
    const first = tokens[0]!;
    const last = tokens[tokens.length - 1]!;
    return normalizeDiagnosticRange(
      {
        startLine: first.line,
        startCol: first.col,
        endLine: last.line,
        endCol: last.col + tokenWidth(last),
      },
      lines,
    );
  }

  function rangeAtLineEnd(
    lines: string[],
    lineIndex: number,
    colHint: number,
  ): ProgramDiagnosticRange {
    const safeLineIndex = Math.max(0, Math.min(lines.length - 1, lineIndex));
    const maxCol = lineLength(lines, safeLineIndex);
    const safeCol = Math.max(0, Math.min(maxCol, colHint));
    if (maxCol > 0) {
      const startCol = Math.max(0, Math.min(maxCol - 1, safeCol));
      return normalizeDiagnosticRange(
        {
          startLine: safeLineIndex,
          startCol,
          endLine: safeLineIndex,
          endCol: startCol + 1,
        },
        lines,
      );
    }
    return normalizeDiagnosticRange(
      {
        startLine: safeLineIndex,
        startCol: 0,
        endLine: safeLineIndex,
        endCol: 1,
      },
      lines,
    );
  }

  function makeDiagnostic(
    kind: "compile" | "ub",
    message: string,
    range: ProgramDiagnosticRange,
    lines: string[],
    tip?: string,
  ): ProgramDiagnostic {
    return {
      kind,
      message,
      tip,
      range: normalizeDiagnosticRange(range, lines),
    };
  }

  function compareDiagnosticLocation(
    left: ProgramDiagnostic,
    right: ProgramDiagnostic,
  ): number {
    if (left.range.startLine !== right.range.startLine) {
      return left.range.startLine - right.range.startLine;
    }
    return left.range.startCol - right.range.startCol;
  }

  function findIdentifierToken(tokens: Token[], name: string): Token | null {
    return (
      tokens.find((token) => token.type === "ident" && token.value === name) || null
    );
  }

  function findUnmatchedDelimiterDiagnostic(
    tokens: Token[],
    lines: string[],
  ): ProgramDiagnostic | null {
    const stack: Token[] = [];
    for (const token of tokens) {
      if (token.type !== "sym") continue;
      if (token.value === "(" || token.value === "[" || token.value === "{") {
        stack.push(token);
        continue;
      }
      if (token.value !== ")" && token.value !== "]" && token.value !== "}") {
        continue;
      }
      const open = stack[stack.length - 1];
      const expected =
        token.value === ")" ? "(" : token.value === "]" ? "[" : "{";
      if (!open || open.value !== expected) {
        return makeDiagnostic(
          "compile",
          `This ${token.value} does not match an earlier opening bracket.`,
          rangeFromToken(token, lines),
          lines,
          "Check that every opening bracket has the same kind of closing bracket.",
        );
      }
      stack.pop();
    }
    const open = stack[stack.length - 1];
    if (!open) return null;
    const close = open.value === "(" ? ")" : open.value === "[" ? "]" : "}";
    return makeDiagnostic(
      "compile",
      `This ${open.value} is missing its closing ${close}.`,
      rangeFromToken(open, lines),
      lines,
      `Add ${close} before the statement ends.`,
    );
  }

  function missingSemicolonDiagnostic(
    lines: string[],
    lineNumber: number,
  ): ProgramDiagnostic {
    const lineIndex = Math.max(0, lineNumber - 1);
    const line = lines[lineIndex] || "";
    let lastCodeCol = 0;
    for (let i = line.length - 1; i >= 0; i--) {
      if (!/\s/.test(line[i] || "")) {
        lastCodeCol = i;
        break;
      }
    }
    return makeDiagnostic(
      "compile",
      "This statement needs a semicolon at the end.",
      rangeAtLineEnd(lines, lineIndex, lastCodeCol),
      lines,
      "Add ; after the last piece of code on this line.",
    );
  }

  function diagnoseUnknownToken(
    token: Token,
    lines: string[],
  ): ProgramDiagnostic {
    if (token.value === "/*") {
      return makeDiagnostic(
        "compile",
        "This block comment never closes.",
        rangeFromToken(token, lines),
        lines,
        "Add */ before the end of the program.",
      );
    }
    if (token.value === "'") {
      return makeDiagnostic(
        "compile",
        "This character literal is unfinished or invalid.",
        rangeFromToken(token, lines),
        lines,
        "Use one character inside single quotes, like 'a' or '\\n'.",
      );
    }
    return makeDiagnostic(
      "compile",
      `I do not know what to do with ${JSON.stringify(token.value)} here.`,
      rangeFromToken(token, lines),
      lines,
    );
  }

  function isTypeDeclarationStart(tokens: Token[]): boolean {
    return !!(
      tokens.length &&
      tokens[0]?.type === "kw" &&
      DIAGNOSTIC_TYPE_KEYWORDS.has(tokens[0].value)
    );
  }

  function levenshteinDistance(left: string, right: string): number {
    const a = String(left || "");
    const b = String(right || "");
    if (a === b) return 0;
    if (!a.length) return b.length;
    if (!b.length) return a.length;
    const prev = new Array(b.length + 1).fill(0);
    const next = new Array(b.length + 1).fill(0);
    for (let j = 0; j <= b.length; j++) prev[j] = j;
    for (let i = 1; i <= a.length; i++) {
      next[0] = i;
      for (let j = 1; j <= b.length; j++) {
        const cost = a[i - 1] === b[j - 1] ? 0 : 1;
        next[j] = Math.min(
          prev[j]! + 1,
          next[j - 1]! + 1,
          prev[j - 1]! + cost,
        );
      }
      for (let j = 0; j <= b.length; j++) prev[j] = next[j]!;
    }
    return prev[b.length]!;
  }

  function closestKnownWord(
    value: string,
    candidates: string[],
    maxDistance: number,
  ): string | null {
    const lower = String(value || "").toLowerCase();
    let best: string | null = null;
    let bestScore: [number, number, number] | null = null;
    for (const candidate of candidates) {
      const distance = levenshteinDistance(lower, candidate.toLowerCase());
      const firstCharPenalty =
        candidate[0]?.toLowerCase() === lower[0]?.toLowerCase() ? 0 : 1;
      const lengthPenalty = Math.abs(candidate.length - lower.length);
      const score: [number, number, number] = [
        distance,
        firstCharPenalty,
        lengthPenalty,
      ];
      if (
        bestScore &&
        (score[0] > bestScore[0] ||
          (score[0] === bestScore[0] && score[1] > bestScore[1]) ||
          (score[0] === bestScore[0] &&
            score[1] === bestScore[1] &&
            score[2] >= bestScore[2]))
      ) {
        continue;
      }
      best = candidate;
      bestScore = score;
    }
    return bestScore && bestScore[0] <= maxDistance ? best : null;
  }

  function looksLikeUnknownTypeDeclaration(tokens: Token[]): boolean {
    if (!tokens.length || tokens[0]?.type !== "ident") return false;
    if (tokens.length === 1) return true;
    if (tokens[1]?.type === "ident") return true;
    if (
      tokens[1]?.type === "sym" &&
      (tokens[1]?.value === "*" || tokens[1]?.value === "(")
    ) {
      return tokens.slice(2).some((token) => token.type === "ident");
    }
    return false;
  }

  function diagnoseUnknownKeywordOrType(
    tokens: Token[],
    lines: string[],
  ): ProgramDiagnostic | null {
    if (!tokens.length || tokens[0]?.type !== "ident") return null;
    const first = tokens[0]!;
    const keywordSuggestion = closestKnownWord(
      first.value,
      ["if", "while", "else"],
      2,
    );
    if (
      keywordSuggestion &&
      (tokens[1]?.value === "(" ||
        tokens[1]?.value === "{" ||
        tokens.length === 1)
    ) {
      return makeDiagnostic(
        "compile",
        `I do not know the keyword ${JSON.stringify(first.value)}. Did you mean ${JSON.stringify(keywordSuggestion)}?`,
        rangeFromToken(first, lines),
        lines,
      );
    }
    if (!looksLikeUnknownTypeDeclaration(tokens)) return null;
    const typeSuggestion = closestKnownWord(
      first.value,
      Array.from(DIAGNOSTIC_TYPE_KEYWORDS),
      2,
    );
    return makeDiagnostic(
      "compile",
      typeSuggestion
        ? `I do not know the type name ${JSON.stringify(first.value)}. Did you mean ${JSON.stringify(typeSuggestion)}?`
        : `I do not know the type name ${JSON.stringify(first.value)}.`,
      rangeFromToken(first, lines),
      lines,
      "The built-in types here are int, long, char, float, double, bool, and pointer versions of those.",
    );
  }

  function diagnoseExpressionSyntax(
    tokens: Token[],
    lines: string[],
  ): ProgramDiagnostic {
    const fallback = makeDiagnostic(
      "compile",
      "This statement does not make sense as written yet.",
      rangeFromTokens(tokens, lines),
      lines,
      "Double-check the punctuation, operators, and variable names.",
    );
    if (!tokens.length) return fallback;
    const first = tokens[0]!;
    const last = tokens[tokens.length - 1]!;
    if (
      last.type === "sym" &&
      (DIAGNOSTIC_BINARY_OPERATORS.has(last.value) ||
        DIAGNOSTIC_PREFIX_OPERATORS.has(last.value))
    ) {
      return makeDiagnostic(
        "compile",
        `This ${last.value} needs something after it.`,
        rangeFromToken(last, lines),
        lines,
        "Finish the expression on the right side of the operator.",
      );
    }
    if (
      last.type === "sym" &&
      (last.value === ")" || last.value === "]" || last.value === "}") &&
      tokens.length >= 2
    ) {
      const previous = tokens[tokens.length - 2]!;
      if (
        previous.type === "sym" &&
        (DIAGNOSTIC_BINARY_OPERATORS.has(previous.value) ||
          DIAGNOSTIC_PREFIX_OPERATORS.has(previous.value))
      ) {
        return makeDiagnostic(
          "compile",
          `This ${previous.value} needs something before the closing ${last.value}.`,
          rangeFromToken(previous, lines),
          lines,
        );
      }
    }
    if (
      first.type === "sym" &&
      (first.value === ")" || first.value === "]" || first.value === "}")
    ) {
      return makeDiagnostic(
        "compile",
        `This expression cannot start with ${first.value}.`,
        rangeFromToken(first, lines),
        lines,
      );
    }
    const parsed = parseExpressionTokens(tokens, 0, { allowVars: true });
    if (!parsed) {
      if (
        first.type === "sym" &&
        DIAGNOSTIC_PREFIX_OPERATORS.has(first.value) &&
        tokens.length === 1
      ) {
        return makeDiagnostic(
          "compile",
          `This ${first.value} needs something after it.`,
          rangeFromToken(first, lines),
          lines,
        );
      }
      if (first.type === "sym" && first.value === "=") {
        return makeDiagnostic(
          "compile",
          "An assignment needs something on the left side of =.",
          rangeFromToken(first, lines),
          lines,
        );
      }
      return fallback;
    }
    if (parsed.nextIndex >= tokens.length) return fallback;
    const unexpected = tokens[parsed.nextIndex]!;
    if (
      unexpected.type === "ident" ||
      unexpected.type === "number" ||
      (unexpected.type === "sym" && unexpected.value === "(")
    ) {
      return makeDiagnostic(
        "compile",
        "There should be an operator before this.",
        rangeFromToken(unexpected, lines),
        lines,
        "For example, you might need +, -, *, /, or a semicolon.",
      );
    }
    if (
      unexpected.type === "sym" &&
      DIAGNOSTIC_BINARY_OPERATORS.has(unexpected.value)
    ) {
      return makeDiagnostic(
        "compile",
        `This ${unexpected.value} needs something on its right.`,
        rangeFromToken(unexpected, lines),
        lines,
      );
    }
    return makeDiagnostic(
      "compile",
      "I got stuck while reading this expression.",
      rangeFromToken(unexpected, lines),
      lines,
      "Double-check the punctuation near this spot.",
    );
  }

  function diagnoseDeclarationSyntax(
    tokens: Token[],
    lines: string[],
  ): ProgramDiagnostic {
    const declHead = parseDeclHead(tokens);
    const fallback = makeDiagnostic(
      "compile",
      "This declaration is malformed.",
      rangeFromTokens(tokens, lines),
      lines,
      "Try something like int count; or int count = 3;.",
    );
    let typeEnd = 0;
    while (
      typeEnd < tokens.length &&
      tokens[typeEnd]?.type === "kw" &&
      DIAGNOSTIC_TYPE_KEYWORDS.has(tokens[typeEnd]!.value)
    ) {
      typeEnd++;
    }
    if (typeEnd >= tokens.length) {
      const token = tokens[Math.max(0, typeEnd - 1)] || tokens[0]!;
      return makeDiagnostic(
        "compile",
        "This declaration needs a variable name after the type.",
        rangeFromToken(token, lines),
        lines,
      );
    }
    if (declHead.kind === "partial" && declHead.hasInitializer) {
      const eqToken = tokens.find((token) => token.type === "sym" && token.value === "=");
      return makeDiagnostic(
        "compile",
        "This assignment needs a value on the right side of =.",
        rangeFromToken(eqToken || tokens[tokens.length - 1]!, lines),
        lines,
      );
    }
    const hasIdentifier = tokens
      .slice(typeEnd)
      .some((token) => token.type === "ident");
    if (!hasIdentifier) {
      return makeDiagnostic(
        "compile",
        "This declaration needs a variable name.",
        rangeFromTokens(tokens.slice(0, Math.max(typeEnd, 1)), lines),
        lines,
      );
    }
    const nameIndex = tokens.findIndex(
      (token, index) => index >= typeEnd && token.type === "ident",
    );
    const assignTokenIndex = tokens.findIndex(
      (token) => token.type === "sym" && token.value === "=",
    );
    if (nameIndex >= 0 && assignTokenIndex < 0) {
      const extraName = tokens.find(
        (token, index) => index > nameIndex && token.type === "ident",
      );
      if (extraName) {
        return makeDiagnostic(
          "compile",
          "This declaration already named one variable. I did not expect another name here.",
          rangeFromToken(extraName, lines),
          lines,
          "End the statement after the variable name, or rewrite the declaration.",
        );
      }
    }
    if (assignTokenIndex >= 0) {
      const rhsTokens = tokens.slice(assignTokenIndex + 1);
      if (!rhsTokens.length) {
        return makeDiagnostic(
          "compile",
          "This assignment needs a value on the right side of =.",
          rangeFromToken(tokens[assignTokenIndex]!, lines),
          lines,
        );
      }
      const rhsParsed = parseExpressionTokens(rhsTokens, 0, { allowVars: true });
      if (!rhsParsed || rhsParsed.nextIndex !== rhsTokens.length) {
        return diagnoseExpressionSyntax(rhsTokens, lines);
      }
    }
    const openBracketIndex = tokens.findIndex(
      (token) => token.type === "sym" && token.value === "[",
    );
    if (openBracketIndex >= 0) {
      let closeBracketIndex = -1;
      for (let i = openBracketIndex + 1; i < tokens.length; i++) {
        if (tokens[i]?.type === "sym" && tokens[i]?.value === "]") {
          closeBracketIndex = i;
          break;
        }
      }
      if (closeBracketIndex < 0) {
        return makeDiagnostic(
          "compile",
          "This array size is missing its closing ].",
          rangeFromToken(tokens[openBracketIndex]!, lines),
          lines,
        );
      }
      const lengthTokens = tokens.slice(openBracketIndex + 1, closeBracketIndex);
      if (!lengthTokens.length) {
        return makeDiagnostic(
          "compile",
          "This array size needs a positive whole number.",
          rangeFromToken(tokens[openBracketIndex]!, lines),
          lines,
        );
      }
      const parsedLength = parseExpressionTokens(lengthTokens, 0, {
        allowVars: false,
      });
      if (!parsedLength || parsedLength.nextIndex !== lengthTokens.length) {
        return makeDiagnostic(
          "compile",
          "This array size needs a positive whole number.",
          rangeFromTokens(lengthTokens, lines),
          lines,
          "Use 1 or larger inside the brackets.",
        );
      }
      const evaluatedLength = evaluateExpression(parsedLength.expr, [], {
        requireValue: true,
      });
      if (
        "error" in evaluatedLength ||
        !isIntegerBase(evaluatedLength.base) ||
        typeof evaluatedLength.value !== "bigint" ||
        evaluatedLength.value <= 0n
      ) {
        return makeDiagnostic(
          "compile",
          "This array size needs a positive whole number.",
          rangeFromTokens(lengthTokens, lines),
          lines,
          "Use 1 or larger inside the brackets.",
        );
      }
    }
    const last = tokens[tokens.length - 1]!;
    if (last.type === "sym" && last.value === "=") {
      return makeDiagnostic(
        "compile",
        "This assignment needs a value on the right side of =.",
        rangeFromToken(last, lines),
        lines,
      );
    }
    return fallback;
  }

  function diagnoseConditionSyntax(
    tokens: Token[],
    keyword: "if" | "while",
    lines: string[],
  ): ProgramDiagnostic {
    const fallback = makeDiagnostic(
      "compile",
      `This ${keyword} statement is malformed.`,
      rangeFromTokens(tokens, lines),
      lines,
      `Write it like ${keyword} (condition).`,
    );
    if (tokens.length === 1) {
      return makeDiagnostic(
        "compile",
        `This ${keyword} needs a condition in parentheses.`,
        rangeFromToken(tokens[0]!, lines),
        lines,
        `Write ${keyword} (condition).`,
      );
    }
    if (tokens[1]?.type !== "sym" || tokens[1]?.value !== "(") {
      return makeDiagnostic(
        "compile",
        `Put the ${keyword} condition inside parentheses.`,
        rangeFromToken(tokens[0]!, lines),
        lines,
        `Write ${keyword} (condition).`,
      );
    }
    const headerEnd = controlHeaderEndIndex(tokens);
    if (headerEnd < 0) {
      return makeDiagnostic(
        "compile",
        "This condition is missing a closing ).",
        rangeFromToken(tokens[1]!, lines),
        lines,
      );
    }
    const exprTokens = tokens.slice(2, headerEnd);
    if (!exprTokens.length) {
      return makeDiagnostic(
        "compile",
        `${keyword} needs a condition between the parentheses.`,
        rangeFromToken(tokens[1]!, lines),
        lines,
      );
    }
    const parsed = parseExpressionTokens(exprTokens, 0, { allowVars: true });
    if (!parsed || parsed.nextIndex !== exprTokens.length) {
      return diagnoseExpressionSyntax(exprTokens, lines);
    }
    return fallback;
  }

  function diagnoseStatementSyntax(
    tokens: Token[],
    lines: string[],
  ): ProgramDiagnostic {
    if (!tokens.length) {
      return makeDiagnostic(
        "compile",
        "This statement is empty.",
        normalizeDiagnosticRange(
          { startLine: 0, startCol: 0, endLine: 0, endCol: 1 },
          lines,
        ),
        lines,
      );
    }
    if (tokens[0]?.type === "kw" && tokens[0].value === "if") {
      return diagnoseConditionSyntax(tokens, "if", lines);
    }
    if (tokens[0]?.type === "kw" && tokens[0].value === "while") {
      return diagnoseConditionSyntax(tokens, "while", lines);
    }
    if (isTypeDeclarationStart(tokens)) {
      return diagnoseDeclarationSyntax(tokens, lines);
    }
    const unknownKeywordOrType = diagnoseUnknownKeywordOrType(tokens, lines);
    if (unknownKeywordOrType) return unknownKeywordOrType;
    return diagnoseExpressionSyntax(tokens, lines);
  }

  function walkExpr(
    expr: ExprNode,
    visit: (node: ExprNode) => boolean | void,
  ): boolean {
    const stop = visit(expr);
    if (stop) return true;
    if (expr.kind === "cast" || expr.kind === "unary" || expr.kind === "postfix") {
      return walkExpr(expr.expr, visit);
    }
    if (expr.kind === "subscript") {
      return walkExpr(expr.left, visit) || walkExpr(expr.index, visit);
    }
    if (expr.kind === "binary" || expr.kind === "assign") {
      return walkExpr(expr.left, visit) || walkExpr(expr.right, visit);
    }
    return false;
  }

  function findFirstUndeclaredVariable(
    expr: ExprNode,
    declared: DeclaredNames,
  ): string | null {
    let missing: string | null = null;
    walkExpr(expr, (node) => {
      if (node.kind !== "var") return false;
      if (declared.has(node.name)) return false;
      missing = node.name;
      return true;
    });
    return missing;
  }

  function coerceScalarResult(
    result: EvalResult | null,
    requireValue: boolean,
  ): ScalarResult {
    if (!result) return makeCompileError();
    if (result.error) return result;
    const source = decayArrayValue(result);
    if (isEvalError(source)) return source;
    if (source.depth !== 0) return makeCompileError();
    const raw = source.value;
    if (source.kind === "lvalue") {
      if (requireValue && String(raw ?? "") === "") return makeUbError();
    }
    const base = source.base || "int";
    if (isFloatingBase(base)) {
      const rawValue = raw ?? "";
      const parsed = parseDoubleValueWithSign(rawValue);
      if (!parsed) return makeCompileError();
      const nanSign =
        "nanSign" in source && source.nanSign !== undefined
          ? source.nanSign
          : parsed.nanSign;
      return { value: parsed.value, base, nanSign };
    }
    try {
      const value = typeof raw === "bigint" ? raw : BigInt(String(raw));
      return { value, base };
    } catch {
      return makeCompileError();
    }
  }

  function evaluateExpressionRaw(
    expr: ExprNode,
    state: BoxState[],
    opts: {
      requireValue?: boolean;
      onSideEffect?: () => void;
    } = {},
  ): EvalResult | EvalError {
    const { requireValue = requireSourceValue, onSideEffect } = opts;
    const by = Object.fromEntries(state.map((b) => [b.name, b]));
    type ArrayInfo = {
      name: string;
      elementType: string;
      elementBase: string;
      elementDepth: number;
      elementPointeeArrayDims: number[];
      elementPointeeInnerDepth: number;
      shape: number[];
      length: number;
      baseAddress: string;
      byIndex: Map<number, BoxState>;
    };
    const arraysByName = (() => {
      const grouped = new Map<
        string,
        {
          elementType: string;
          elementBase: string;
          elementDepth: number;
          elementPointeeArrayDims: number[];
          elementPointeeInnerDepth: number;
          shape: number[];
          byIndex: Map<number, BoxState>;
        }
      >();
      for (const box of state) {
        let baseName = "";
        let indices: number[] = [];
        const metadataShape = normalizeArrayDims(box.arrayShape);
        if (box.arrayRoot && Array.isArray(box.arrayIndices) && box.arrayIndices.length > 0) {
          baseName = String(box.arrayRoot);
          indices = box.arrayIndices
            .map((raw) => Math.floor(Number(raw)))
            .filter((value) => Number.isFinite(value) && value >= 0);
        } else {
          const parsedName = parseArrayElementName(box.name);
          if (!parsedName) continue;
          baseName = parsedName.baseName;
          indices = parsedName.indices;
        }
        if (!baseName || !indices.length) continue;
        const linearIndex =
          arrayLinearIndex(indices, metadataShape) ??
          (indices.length === 1 ? indices[0]! : null);
        if (linearIndex == null || linearIndex < 0) continue;
        const parsedType = parseType(box.type);
        if (!parsedType.base) continue;
        const elementPointeeArrayDims = normalizeArrayDims(box.pointeeArrayDims);
        const elementPointeeInnerDepth = normalizePointeeInnerDepth(
          box.pointeeInnerDepth ?? parsedType.pointeeInnerDepth,
          parsedType.depth,
          elementPointeeArrayDims,
        );
        const existing = grouped.get(baseName);
        if (!existing) {
          grouped.set(baseName, {
            elementType: box.type,
            elementBase: parsedType.base,
            elementDepth: parsedType.depth,
            elementPointeeArrayDims,
            elementPointeeInnerDepth,
            shape: metadataShape,
            byIndex: new Map([[linearIndex!, box]]),
          });
          continue;
        }
        if (
          existing.elementType !== box.type ||
          existing.elementBase !== parsedType.base ||
          existing.elementDepth !== parsedType.depth ||
          !sameArrayDims(existing.elementPointeeArrayDims, elementPointeeArrayDims) ||
          existing.elementPointeeInnerDepth !== elementPointeeInnerDepth
        ) {
          continue;
        }
        if (!existing.shape.length && metadataShape.length) {
          existing.shape = metadataShape;
        } else if (
          existing.shape.length &&
          metadataShape.length &&
          !sameArrayDims(existing.shape, metadataShape)
        ) {
          continue;
        }
        existing.byIndex.set(linearIndex!, box);
      }
      const out = new Map<string, ArrayInfo>();
      for (const [name, entry] of grouped.entries()) {
        const first = entry.byIndex.get(0);
        if (!first || !String(first.address ?? "").trim()) continue;
        let shape = entry.shape.slice();
        let length = 0;
        if (shape.length) {
          length = arrayElementCount(shape);
        } else {
          while (entry.byIndex.has(length)) length++;
          if (length > 0) shape = [length];
        }
        if (length <= 0) continue;
        out.set(name, {
          name,
          elementType: entry.elementType,
          elementBase: entry.elementBase,
          elementDepth: entry.elementDepth,
          elementPointeeArrayDims: entry.elementPointeeArrayDims.slice(),
          elementPointeeInnerDepth: entry.elementPointeeInnerDepth,
          shape: shape.slice(),
          length,
          baseAddress: String(first.address ?? "").trim(),
          byIndex: entry.byIndex,
        });
      }
      return out;
    })();
    const toNumber = (value: bigint | number): number =>
      typeof value === "bigint" ? Number(value) : value;
    const toUnsignedBits = (value: bigint, base: string): bigint => {
      const width = bitWidthForBase(base);
      if (!width) return value;
      const modulo = 1n << BigInt(width);
      let next = value % modulo;
      if (next < 0n) next += modulo;
      return next;
    };
    const pointerStepSize = (
      base: string,
      depth: number,
      pointeeArrayDims: number[] = [],
      pointeeInnerDepth?: number,
    ): bigint => {
      const pointeeDepth = Math.max(0, depth - 1);
      const pointeeType =
        makePointerType(
          pointeeDepth,
          base,
          pointeeArrayDims,
          pointeeInnerDepth,
        ) || base;
      const info = typeInfo(pointeeType);
      const size = info.size > 0 ? info.size : 1;
      return BigInt(size);
    };

    function makeLvalue(box: BoxState, label: string): EvalResult {
      const {
        base,
        depth,
        pointeeArrayDims: parsedPointeeArrayDims,
        pointeeInnerDepth: parsedPointeeInnerDepth,
      } = parseType(box.type);
      if (!base) return makeCompileError();
      const pointeeArrayDims = (() => {
        const fromBox = normalizeArrayDims(box.pointeeArrayDims);
        if (fromBox.length) return fromBox;
        return normalizeArrayDims(parsedPointeeArrayDims);
      })();
      const pointeeInnerDepth = normalizePointeeInnerDepth(
        box.pointeeInnerDepth ?? parsedPointeeInnerDepth,
        depth,
        pointeeArrayDims,
      );
      let nanSign: -1 | 1 | undefined;
      if (isFloatingBase(base)) {
        const parsed = parseDoubleValueWithSign(box.value);
        nanSign = parsed?.nanSign;
      }
      return {
        kind: "lvalue",
        base,
        depth,
        value: box.value,
        address: box.address ?? "",
        label: label || box.name,
        nanSign,
        pointeeArrayDims,
        pointeeInnerDepth,
      };
    }

    function makeArrayLvalue(info: ArrayInfo, label: string): EvalResult {
      let ptr: bigint;
      try {
        ptr = BigInt(String(info.baseAddress || "0").trim() || "0");
      } catch {
        return makeCompileError();
      }
      return {
        kind: "lvalue",
        base: info.elementBase,
        depth: info.elementDepth + 1,
        value: ptr,
        address: String(info.baseAddress || ""),
        label,
        isArray: true,
        arrayShape: info.shape.slice(),
        pointeeArrayDims: info.shape.length > 1
          ? info.shape.slice(1)
          : info.elementPointeeArrayDims.slice(),
        pointeeInnerDepth: info.shape.length > 1
          ? Math.max(0, info.elementDepth)
          : normalizePointeeInnerDepth(
              info.elementPointeeInnerDepth,
              info.elementDepth + 1,
              info.elementPointeeArrayDims,
            ),
      };
    }

    function makeRvalue(
      value: BoxValue | bigint | number,
      base: string,
      depth: number = 0,
      label: string = "",
      nanSign?: -1 | 1,
      pointeeArrayDims: number[] = [],
      pointeeInnerDepth?: number,
    ): EvalResult {
      const normalizedPointeeArrayDims = normalizeArrayDims(pointeeArrayDims);
      return {
        kind: "rvalue",
        base,
        depth,
        value,
        address: "",
        label,
        nanSign,
        pointeeArrayDims: normalizedPointeeArrayDims,
        pointeeInnerDepth: normalizePointeeInnerDepth(
          pointeeInnerDepth,
          depth,
          normalizedPointeeArrayDims,
        ),
      };
    }

    type RuntimeValue = {
      base: string;
      depth: number;
      value: bigint | number;
      nanSign?: -1 | 1;
      label: string;
      address: string;
      kind: "lvalue" | "rvalue";
      pointeeArrayDims: number[];
      pointeeInnerDepth: number;
    };

    function runtimeFromEval(
      evaluated: EvalResult,
      mustHaveValue: boolean = requireValue,
    ): RuntimeValue | EvalError {
      if (isEvalError(evaluated)) return evaluated;
      const decayed = decayArrayValue(evaluated);
      if (isEvalError(decayed)) return decayed;
      const raw = decayed.value;
      if (mustHaveValue && String(raw ?? "") === "") {
        return makeUbError();
      }
      if (decayed.depth > 0) {
        const trimmed = String(raw ?? "").trim();
        if (!trimmed) {
          return makeUbError();
        }
        try {
          return {
            base: decayed.base,
            depth: decayed.depth,
            value: BigInt(trimmed),
            nanSign: decayed.nanSign,
            label: decayed.label || "",
            address: decayed.address || "",
            kind: decayed.kind,
            pointeeArrayDims: normalizeArrayDims(decayed.pointeeArrayDims),
            pointeeInnerDepth: normalizePointeeInnerDepth(
              decayed.pointeeInnerDepth,
              decayed.depth,
              normalizeArrayDims(decayed.pointeeArrayDims),
            ),
          };
        } catch {
          return makeCompileError();
        }
      }
      if (isFloatingBase(decayed.base)) {
        const parsed = parseDoubleValueWithSign(raw);
        if (!parsed) {
          return makeCompileError();
        }
        return {
          base: decayed.base,
          depth: 0,
          value: parsed.value,
          nanSign:
            decayed.nanSign !== undefined ? decayed.nanSign : parsed.nanSign,
          label: decayed.label || "",
          address: decayed.address || "",
          kind: decayed.kind,
          pointeeArrayDims: [],
          pointeeInnerDepth: 0,
        };
      }
      if (!isIntegerBase(decayed.base)) {
        return makeCompileError();
      }
      try {
        const parsedInt =
          typeof raw === "bigint" ? raw : BigInt(String(raw ?? "").trim() || "0");
        const normalized = normalizeIntegerForBase(parsedInt, decayed.base);
        if (normalized == null) {
          return makeCompileError();
        }
        return {
          base: decayed.base,
          depth: 0,
          value: normalized,
          nanSign: decayed.nanSign,
          label: decayed.label || "",
          address: decayed.address || "",
          kind: decayed.kind,
          pointeeArrayDims: [],
          pointeeInnerDepth: 0,
        };
      } catch {
        return makeCompileError();
      }
    }

    function runtimeTruthy(value: RuntimeValue): boolean {
      if (value.depth > 0) return (value.value as bigint) !== 0n;
      if (isFloatingBase(value.base)) {
        const num = value.value as number;
        return num !== 0;
      }
      return (value.value as bigint) !== 0n;
    }

    function resolveTargetBox(target: EvalResult): BoxState | null {
      if (isEvalError(target)) return null;
      if (target.kind !== "lvalue") return null;
      return (
        state.find((box) => String(box.address ?? "") === String(target.address ?? "")) ||
        null
      );
    }

    function pointerCanReferenceTarget(
      pointer: {
        base: string;
        depth: number;
        pointeeArrayDims?: number[];
        pointeeInnerDepth?: number;
      },
      target: BoxState,
    ): boolean {
      const pointerBase = pointer.base || "int";
      const pointerDepth = Math.floor(pointer.depth);
      if (pointerDepth < 1) return false;
      const pointerPointeeArrayDims = normalizeArrayDims(pointer.pointeeArrayDims);
      const pointerPointeeInnerDepth = normalizePointeeInnerDepth(
        pointer.pointeeInnerDepth,
        pointerDepth,
        pointerPointeeArrayDims,
      );
      const pointerOuterDepth = Math.max(0, pointerDepth - pointerPointeeInnerDepth);

      const parsedTarget = parseType(target.type || "int");
      if (!parsedTarget.base) return false;
      if (parsedTarget.base !== pointerBase) return false;
      const targetDepth = Math.floor(parsedTarget.depth);
      const targetPointeeArrayDims = (() => {
        const fromBox = normalizeArrayDims(target.pointeeArrayDims);
        if (fromBox.length) return fromBox;
        return normalizeArrayDims(parsedTarget.pointeeArrayDims);
      })();
      const targetPointeeInnerDepth = normalizePointeeInnerDepth(
        target.pointeeInnerDepth ?? parsedTarget.pointeeInnerDepth,
        targetDepth,
        targetPointeeArrayDims,
      );

      if (!pointerPointeeArrayDims.length) {
        return targetDepth === pointerDepth - 1 && targetPointeeArrayDims.length === 0;
      }
      if (pointerOuterDepth === 1) {
        return (
          targetDepth === pointerPointeeInnerDepth &&
          targetPointeeArrayDims.length === 0
        );
      }
      return (
        targetDepth === pointerDepth - 1 &&
        samePointerPointeeType(
          targetDepth,
          targetPointeeArrayDims,
          targetPointeeInnerDepth,
          pointerDepth - 1,
          pointerPointeeArrayDims,
          pointerPointeeInnerDepth,
        )
      );
    }

    function assignIntoTarget(
      target: EvalResult,
      source: EvalResult,
    ): EvalResult | EvalError {
      if (isEvalError(target)) return target;
      if (target.kind !== "lvalue") {
        return makeCompileError();
      }
      if (target.isArray) {
        return makeCompileError();
      }
      const targetType =
        makePointerType(
          target.depth,
          target.base || "int",
          normalizeArrayDims(target.pointeeArrayDims),
          target.pointeeInnerDepth,
        ) || "int";
      const converted = convertAssignmentValue(source, targetType, requireValue);
      if ("kind" in converted && converted.kind === "type-mismatch") {
        return makeCompileError();
      }
      if ("error" in converted) return converted;
      const box = resolveTargetBox(target);
      if (!box) return makeCompileError();
      box.value = converted.value;
      onSideEffect?.();
      if (target.depth > 0) {
        const nextAddress = String(box.value ?? "").trim();
        try {
          return makeRvalue(
            BigInt(nextAddress || "0"),
            target.base,
            target.depth,
            target.label,
            converted.nanSign,
            normalizeArrayDims(target.pointeeArrayDims),
            target.pointeeInnerDepth,
          );
        } catch {
          return makeCompileError();
        }
      }
      if (isFloatingBase(target.base)) {
        const parsed = parseDoubleValueWithSign(box.value);
        if (!parsed) return makeCompileError();
        return makeRvalue(
          parsed.value,
          target.base,
          0,
          target.label,
          converted.nanSign ?? parsed.nanSign,
        );
      }
      try {
        const parsed = BigInt(String(box.value ?? "").trim() || "0");
        const normalized = normalizeIntegerForBase(parsed, target.base);
        if (normalized == null) return makeCompileError();
        return makeRvalue(normalized, target.base, 0, target.label);
      } catch {
        return makeCompileError();
      }
    }

    function evaluateBinaryResolved(
      op: BinaryOp,
      leftEval: EvalResult,
      rightEval: EvalResult,
    ): EvalResult | EvalError {
      const left = runtimeFromEval(leftEval, true);
      if (isEvalError(left)) return left;
      const right = runtimeFromEval(rightEval, true);
      if (isEvalError(right)) return right;

      if (op === "==" || op === "!=") {
        if (left.depth > 0 || right.depth > 0) {
          let result = false;
          if (left.depth > 0 && right.depth > 0) {
            if (
              left.base !== right.base ||
              left.depth !== right.depth ||
              !samePointerPointeeType(
                left.depth,
                left.pointeeArrayDims,
                left.pointeeInnerDepth,
                right.depth,
                right.pointeeArrayDims,
                right.pointeeInnerDepth,
              )
            ) {
              return makeCompileError();
            }
            result = (left.value as bigint) === (right.value as bigint);
          } else {
            const pointerValue = left.depth > 0 ? (left.value as bigint) : (right.value as bigint);
            const scalar = left.depth > 0 ? right : left;
            if (scalar.depth !== 0 || !isIntegerBase(scalar.base)) {
              return makeCompileError();
            }
            result = pointerValue === (scalar.value as bigint);
          }
          return makeRvalue(result === (op === "==") ? 1n : 0n, "int");
        }
      }

      if (op === "&&" || op === "||") {
        const leftTruthy = runtimeTruthy(left);
        const rightTruthy = runtimeTruthy(right);
        const value =
          op === "&&" ? leftTruthy && rightTruthy : leftTruthy || rightTruthy;
        return makeRvalue(value ? 1n : 0n, "int");
      }

      if (op === "+" || op === "-") {
        if (left.depth > 0 || right.depth > 0) {
          if (op === "+" && left.depth > 0 && right.depth > 0) {
            return makeCompileError();
          }
          if (
            op === "-" &&
            left.depth > 0 &&
            right.depth > 0
          ) {
            if (left.base !== right.base || left.depth !== right.depth) {
              return makeCompileError();
            }
            if (
              !samePointerPointeeType(
                left.depth,
                left.pointeeArrayDims,
                left.pointeeInnerDepth,
                right.depth,
                right.pointeeArrayDims,
                right.pointeeInnerDepth,
              )
            ) {
              return makeCompileError();
            }
            const step = pointerStepSize(
              left.base,
              left.depth,
              left.pointeeArrayDims,
              left.pointeeInnerDepth,
            );
            if (step === 0n) return makeCompileError();
            const diff = ((left.value as bigint) - (right.value as bigint)) / step;
            const normalized = normalizeIntegerForBase(diff, "long");
            if (normalized == null) return makeCompileError();
            return makeRvalue(normalized, "long");
          }
          const pointer = left.depth > 0 ? left : right;
          const scalar = left.depth > 0 ? right : left;
          if (scalar.depth !== 0 || !isIntegerBase(scalar.base)) {
            return makeCompileError();
          }
          const step = pointerStepSize(
            pointer.base,
            pointer.depth,
            pointer.pointeeArrayDims,
            pointer.pointeeInnerDepth,
          );
          const delta = scalar.value as bigint;
          const signedDelta = left.depth > 0
            ? op === "+" ? delta : -delta
            : delta;
          const nextAddress = (pointer.value as bigint) + signedDelta * step;
          return makeRvalue(
            nextAddress,
            pointer.base,
            pointer.depth,
            "",
            undefined,
            pointer.pointeeArrayDims,
            pointer.pointeeInnerDepth,
          );
        }
      }

      if (
        (op === "<" || op === "<=" || op === ">" || op === ">=") &&
        (left.depth > 0 || right.depth > 0)
      ) {
        if (left.depth <= 0 || right.depth <= 0) {
          return makeCompileError();
        }
        if (
          left.base !== right.base ||
          left.depth !== right.depth ||
          !samePointerPointeeType(
            left.depth,
            left.pointeeArrayDims,
            left.pointeeInnerDepth,
            right.depth,
            right.pointeeArrayDims,
            right.pointeeInnerDepth,
          )
        ) {
          return makeCompileError();
        }
        const lhs = left.value as bigint;
        const rhs = right.value as bigint;
        const ok =
          op === "<"
            ? lhs < rhs
            : op === "<="
              ? lhs <= rhs
              : op === ">"
                ? lhs > rhs
                : lhs >= rhs;
        return makeRvalue(ok ? 1n : 0n, "int");
      }

      if (left.depth > 0 || right.depth > 0) {
        return makeCompileError();
      }

      const useFloat = isFloatingBase(left.base) || isFloatingBase(right.base);
      if (useFloat) {
        if (
          op === "&" ||
          op === "|" ||
          op === "^" ||
          op === "<<" ||
          op === ">>" ||
          op === "%" 
        ) {
          return makeCompileError();
        }
        const resultBase =
          left.base === "double" || right.base === "double" ? "double" : "float";
        const lhs = toNumber(left.value);
        const rhs = toNumber(right.value);
        if (op === "==" || op === "!=" || op === "<" || op === "<=" || op === ">" || op === ">=") {
          let ok = false;
          if (Number.isNaN(lhs) || Number.isNaN(rhs)) {
            ok = op === "!=";
          } else if (op === "==") ok = lhs === rhs;
          else if (op === "!=") ok = lhs !== rhs;
          else if (op === "<") ok = lhs < rhs;
          else if (op === "<=") ok = lhs <= rhs;
          else if (op === ">") ok = lhs > rhs;
          else ok = lhs >= rhs;
          return makeRvalue(ok ? 1n : 0n, "int");
        }
        let out: number;
        if (op === "+") out = lhs + rhs;
        else if (op === "-") out = lhs - rhs;
        else if (op === "*") out = lhs * rhs;
        else if (op === "/") out = lhs / rhs;
        else return makeCompileError();
        const cast = resultBase === "float" ? Math.fround(out) : out;
        const nanSign = Number.isNaN(cast)
          ? left.nanSign ?? right.nanSign ?? 1
          : undefined;
        return makeRvalue(cast, resultBase, 0, "", nanSign);
      }

      if (!isIntegerBase(left.base) || !isIntegerBase(right.base)) {
        return makeCompileError();
      }
      const leftBase =
        op === "<<" || op === ">>"
          ? integerPromotionBase(left.base)
          : usualIntegerBase(left.base, right.base);
      const rightBase =
        op === "<<" || op === ">>"
          ? integerPromotionBase(right.base)
          : leftBase;
      const leftNorm = normalizeIntegerForBase(left.value as bigint, leftBase);
      const rightNorm = normalizeIntegerForBase(right.value as bigint, rightBase);
      if (leftNorm == null || rightNorm == null) {
        return makeCompileError();
      }
      const leftMeta = integerMetaForBase(leftBase);
      const rightMeta = integerMetaForBase(rightBase);
      if (!leftMeta || !rightMeta) {
        return makeCompileError();
      }

      if (op === "==" || op === "!=" || op === "<" || op === "<=" || op === ">" || op === ">=") {
        const common = usualIntegerBase(left.base, right.base);
        const lhs = normalizeIntegerForBase(left.value as bigint, common);
        const rhs = normalizeIntegerForBase(right.value as bigint, common);
        if (lhs == null || rhs == null) return makeCompileError();
        let ok = false;
        if (op === "==") ok = lhs === rhs;
        else if (op === "!=") ok = lhs !== rhs;
        else if (op === "<") ok = lhs < rhs;
        else if (op === "<=") ok = lhs <= rhs;
        else if (op === ">") ok = lhs > rhs;
        else ok = lhs >= rhs;
        return makeRvalue(ok ? 1n : 0n, "int");
      }

      if (op === "/" || op === "%") {
        if (rightNorm === 0n) return makeUbError();
      }

      if (op === "<<" || op === ">>") {
        const width = bitWidthForBase(leftBase);
        if (!width) return makeCompileError();
        if (rightNorm < 0n || rightNorm >= BigInt(width)) {
          return makeUbError();
        }
        if (op === "<<") {
          if (leftMeta.signed && leftNorm < 0n) {
            return makeUbError();
          }
          if (leftMeta.signed) {
            const shifted = leftNorm << rightNorm;
            const overflow = checkIntegerRange(shifted, leftBase);
            if (overflow) return overflow;
            return makeRvalue(shifted, leftBase);
          }
          const shiftedBits = toUnsignedBits(leftNorm, leftBase) << rightNorm;
          const wrapped = normalizeIntegerForBase(shiftedBits, leftBase);
          if (wrapped == null) return makeCompileError();
          return makeRvalue(wrapped, leftBase);
        }
        if (!leftMeta.signed) {
          const shifted = toUnsignedBits(leftNorm, leftBase) >> rightNorm;
          const wrapped = normalizeIntegerForBase(shifted, leftBase);
          if (wrapped == null) return makeCompileError();
          return makeRvalue(wrapped, leftBase);
        }
        const shifted = leftNorm >> rightNorm;
        const wrapped = normalizeIntegerForBase(shifted, leftBase);
        if (wrapped == null) return makeCompileError();
        return makeRvalue(wrapped, leftBase);
      }

      if (op === "&" || op === "^" || op === "|") {
        const common = usualIntegerBase(left.base, right.base);
        const lhs = normalizeIntegerForBase(left.value as bigint, common);
        const rhs = normalizeIntegerForBase(right.value as bigint, common);
        if (lhs == null || rhs == null) return makeCompileError();
        const lhsBits = toUnsignedBits(lhs, common);
        const rhsBits = toUnsignedBits(rhs, common);
        const outBits =
          op === "&" ? lhsBits & rhsBits : op === "^" ? lhsBits ^ rhsBits : lhsBits | rhsBits;
        const wrapped = normalizeIntegerForBase(outBits, common);
        if (wrapped == null) return makeCompileError();
        return makeRvalue(wrapped, common);
      }

      const common = usualIntegerBase(left.base, right.base);
      const lhs = normalizeIntegerForBase(left.value as bigint, common);
      const rhs = normalizeIntegerForBase(right.value as bigint, common);
      if (lhs == null || rhs == null) return makeCompileError();
      const commonMeta = integerMetaForBase(common);
      if (!commonMeta) return makeCompileError();
      let out: bigint;
      if (op === "+") out = lhs + rhs;
      else if (op === "-") out = lhs - rhs;
      else if (op === "*") out = lhs * rhs;
      else if (op === "/") {
        if (commonMeta.signed && rhs === -1n) {
          const range = integerRangeForBase(common);
          if (range && lhs === range.min) return integerOverflowError(common);
        }
        if (commonMeta.signed) out = lhs / rhs;
        else out = toUnsignedBits(lhs, common) / toUnsignedBits(rhs, common);
      } else if (op === "%") {
        if (commonMeta.signed) out = lhs % rhs;
        else out = toUnsignedBits(lhs, common) % toUnsignedBits(rhs, common);
      } else {
        return makeCompileError();
      }
      if (commonMeta.signed) {
        const overflow = checkIntegerRange(out, common);
        if (overflow) return overflow;
      }
      const normalized = normalizeIntegerForBase(out, common);
      if (normalized == null) return makeCompileError();
      return makeRvalue(normalized, common);
    }

    function incrementLvalue(
      target: EvalResult,
      delta: 1 | -1,
      returnUpdated: boolean,
    ): EvalResult | EvalError {
      if (isEvalError(target)) return target;
      if (target.kind !== "lvalue") {
        return makeCompileError();
      }
      if (target.isArray) {
        return makeCompileError();
      }
      const current = runtimeFromEval(target, true);
      if (isEvalError(current)) return current;
      let updated: EvalResult | EvalError;
      if (current.depth > 0) {
        const step = pointerStepSize(
          current.base,
          current.depth,
          current.pointeeArrayDims,
          current.pointeeInnerDepth,
        );
        const next = (current.value as bigint) + BigInt(delta) * step;
        updated = makeRvalue(
          next,
          current.base,
          current.depth,
          "",
          undefined,
          current.pointeeArrayDims,
          current.pointeeInnerDepth,
        );
      } else if (isFloatingBase(current.base)) {
        const nextNum = toNumber(current.value) + delta;
        const cast = current.base === "float" ? Math.fround(nextNum) : nextNum;
        updated = makeRvalue(cast, current.base, 0, "", current.nanSign);
      } else if (isIntegerBase(current.base)) {
        const nextRaw = (current.value as bigint) + BigInt(delta);
        const range = integerRangeForBase(current.base);
        if (range?.signed) {
          const overflow = checkIntegerRange(nextRaw, current.base);
          if (overflow) return overflow;
        }
        const wrapped = normalizeIntegerForBase(nextRaw, current.base);
        if (wrapped == null) return makeCompileError();
        updated = makeRvalue(wrapped, current.base);
      } else {
        return makeCompileError();
      }
      if (isEvalError(updated)) return updated;
      const assigned = assignIntoTarget(target, updated);
      if (isEvalError(assigned)) return assigned;
      if (returnUpdated) return assigned;
      return makeRvalue(
        current.value,
        current.base,
        current.depth,
        current.label,
        current.nanSign,
        current.pointeeArrayDims,
        current.pointeeInnerDepth,
      );
    }

    function evalNode(node: ExprNode | null): EvalResult {
      if (!node) return makeCompileError();
      if (node.kind === "num") {
        const parsed = parseNumericLiteralValue(node.value);
        if ("error" in parsed) return parsed;
        return makeRvalue(parsed.value, parsed.base, 0, "", parsed.nanSign);
      }
      if (node.kind === "cast") {
        const target = parseType(node.targetType || "int");
        const targetBase = target.base;
        const targetDepth = target.depth;
        const targetPointeeArrayDims = normalizeArrayDims(target.pointeeArrayDims);
        const targetPointeeInnerDepth = normalizePointeeInnerDepth(
          target.pointeeInnerDepth,
          targetDepth,
          targetPointeeArrayDims,
        );
        if (!targetBase) {
          return makeCompileError();
        }
        const rhs = evalNode(node.expr);
        if (isEvalError(rhs)) return rhs;
        const source = decayArrayValue(rhs);
        if (isEvalError(source)) return source;
        if (targetDepth > 0) {
          if (source.depth > 0) {
            const runtime = runtimeFromEval(source, requireValue);
            if (isEvalError(runtime)) return runtime;
            return makeRvalue(
              runtime.value as bigint,
              targetBase,
              targetDepth,
              "",
              runtime.nanSign,
              targetPointeeArrayDims,
              targetPointeeInnerDepth,
            );
          }
          const scalar = coerceScalarResult(source, requireValue);
          if (isScalarError(scalar)) return scalar;
          if (!isIntegerBase(scalar.base)) {
            return makeCompileError();
          }
          const asInt =
            typeof scalar.value === "bigint"
              ? scalar.value
              : BigInt(Math.trunc(Number(scalar.value)));
          return makeRvalue(
            asInt,
            targetBase,
            targetDepth,
            "",
            scalar.nanSign,
            targetPointeeArrayDims,
            targetPointeeInnerDepth,
          );
        }
        if (source.depth > 0) {
          const runtime = runtimeFromEval(source, requireValue);
          if (isEvalError(runtime)) return runtime;
          if (isFloatingBase(targetBase)) {
            const num = Number(runtime.value as bigint);
            const castNum = targetBase === "float" ? Math.fround(num) : num;
            return makeRvalue(castNum, targetBase, 0);
          }
          if (!isIntegerBase(targetBase)) {
            return makeCompileError();
          }
          const converted = convertScalarForAssignment(
            runtime.value as bigint,
            "unsigned long long",
            node.targetType,
            runtime.nanSign,
          );
          if (!converted || "error" in converted) {
            return makeCompileError();
          }
          return makeRvalue(
            converted.value,
            targetBase,
            0,
            "",
            converted.nanSign,
          );
        }
        const scalar = coerceScalarResult(source, requireValue);
        if (isScalarError(scalar)) return scalar;
        const converted = convertScalarForAssignment(
          scalar.value,
          scalar.base,
          node.targetType,
          scalar.nanSign,
        );
        if (!converted || "error" in converted) {
          return makeCompileError();
        }
        return makeRvalue(
          converted.value,
          targetBase,
          0,
          "",
          converted.nanSign,
        );
      }
      if (node.kind === "var") {
        const box = by[node.name];
        if (box) return makeLvalue(box, node.name);
        const arrayInfo = arraysByName.get(node.name);
        if (arrayInfo) return makeArrayLvalue(arrayInfo, node.name);
        return makeCompileError();
      }
      if (node.kind === "postfix") {
        if (node.op === "++") return incrementLvalue(evalNode(node.expr), 1, false);
        return incrementLvalue(evalNode(node.expr), -1, false);
      }
      if (node.kind === "subscript") {
        const left = evalNode(node.left);
        if (isEvalError(left)) return left;
        const index = evalNode(node.index);
        if (isEvalError(index)) return index;
        const leftRuntime = runtimeFromEval(left, true);
        if (isEvalError(leftRuntime)) return leftRuntime;
        const indexRuntime = runtimeFromEval(index, true);
        if (isEvalError(indexRuntime)) return indexRuntime;
        let pointer = leftRuntime;
        let scalar = indexRuntime;
        if (!(pointer.depth > 0 && scalar.depth === 0 && isIntegerBase(scalar.base))) {
          if (
            indexRuntime.depth > 0 &&
            leftRuntime.depth === 0 &&
            isIntegerBase(leftRuntime.base)
          ) {
            pointer = indexRuntime;
            scalar = leftRuntime;
          } else {
            return makeCompileError();
          }
        }
        const step = pointerStepSize(
          pointer.base,
          pointer.depth,
          pointer.pointeeArrayDims,
          pointer.pointeeInnerDepth,
        );
        const nextAddress = (pointer.value as bigint) + (scalar.value as bigint) * step;
        const target = state.find(
          (box) => String(box.address ?? "").trim() === String(nextAddress),
        );
        if (!target) return makeUbError();
        if (!pointerCanReferenceTarget(pointer, target)) return makeUbError();
        if (pointer.pointeeArrayDims.length > 0) {
          const shape = pointer.pointeeArrayDims.slice();
          const targetParsedType = parseType(target.type);
          const targetPointeeInnerDepth = normalizePointeeInnerDepth(
            target.pointeeInnerDepth ?? targetParsedType.pointeeInnerDepth,
            targetParsedType.depth,
            normalizeArrayDims(target.pointeeArrayDims),
          );
          const decayDims =
            shape.length > 1
              ? shape.slice(1)
              : normalizeArrayDims(target.pointeeArrayDims);
          return {
            kind: "lvalue",
            base: pointer.base,
            depth: pointer.depth,
            value: nextAddress,
            address: String(nextAddress),
            label: `${left.label || ""}[${index.label || ""}]`,
            isArray: true,
            arrayShape: shape,
            pointeeArrayDims: decayDims,
            pointeeInnerDepth:
              shape.length > 1
                ? Math.max(0, pointer.depth - 1)
                : targetPointeeInnerDepth,
          };
        }
        return makeLvalue(target, `${left.label || ""}[${index.label || ""}]`);
      }
      if (node.kind === "unary") {
        if (node.op === "++") return incrementLvalue(evalNode(node.expr), 1, true);
        if (node.op === "--") return incrementLvalue(evalNode(node.expr), -1, true);
        const rhs = evalNode(node.expr);
        if (isEvalError(rhs)) return rhs;
        const rhsDepth = rhs.depth ?? 0;

        if (node.op === "&") {
          const label = `&${rhs.label || ""}`;
          if (rhs.kind !== "lvalue") return makeCompileError();
          if (rhs.isArray) {
            const shape = normalizeArrayDims(rhs.arrayShape);
            if (!shape.length) return makeCompileError();
            return makeRvalue(
              rhs.value,
              rhs.base || "int",
              rhsDepth,
              label,
              rhs.nanSign,
              shape,
              Math.max(0, rhsDepth - 1),
            );
          }
          if (!rhs.address) return makeCompileError();
          const nextDepth = rhsDepth + 1;
          const nextBase = rhs.base || "int";
          return makeRvalue(
            String(rhs.address),
            nextBase,
            nextDepth,
            label,
            rhs.nanSign,
            normalizeArrayDims(rhs.pointeeArrayDims),
            rhs.pointeeInnerDepth,
          );
        }
        if (node.op === "*") {
          const label = `*${rhs.label || ""}`;
          if (rhsDepth < 1) {
            return makeCompileError();
          }
          const ptrRaw = rhs.value;
          if (requireValue && String(ptrRaw ?? "") === "") return makeUbError();
          const ptrVal = String(ptrRaw ?? "").trim();
          if (ptrVal === "") return makeUbError();
          const target = state.find((b) => (b.address ?? "") === ptrVal);
          if (!target) return makeUbError();
          if (
            !pointerCanReferenceTarget(
              {
                base: rhs.base || "int",
                depth: rhsDepth,
                pointeeArrayDims: normalizeArrayDims(rhs.pointeeArrayDims),
                pointeeInnerDepth: rhs.pointeeInnerDepth,
              },
              target,
            )
          ) {
            return makeUbError();
          }
          const pointeeArrayDims = normalizeArrayDims(rhs.pointeeArrayDims);
          if (pointeeArrayDims.length > 0) {
            const targetParsedType = parseType(target.type);
            const targetPointeeInnerDepth = normalizePointeeInnerDepth(
              target.pointeeInnerDepth ?? targetParsedType.pointeeInnerDepth,
              targetParsedType.depth,
              normalizeArrayDims(target.pointeeArrayDims),
            );
            return {
              kind: "lvalue",
              base: rhs.base || "int",
              depth: rhsDepth,
              value: BigInt(ptrVal),
              address: ptrVal,
              label,
              isArray: true,
              arrayShape: pointeeArrayDims.slice(),
              pointeeArrayDims:
                pointeeArrayDims.length > 1
                  ? pointeeArrayDims.slice(1)
                  : normalizeArrayDims(target.pointeeArrayDims),
              pointeeInnerDepth:
                pointeeArrayDims.length > 1
                  ? Math.max(0, rhsDepth - 1)
                  : targetPointeeInnerDepth,
            };
          }
          return makeLvalue(target, label);
        }
        if (node.op === "!") {
          const runtime = runtimeFromEval(rhs, true);
          if (isEvalError(runtime)) return runtime;
          return makeRvalue(runtimeTruthy(runtime) ? 0n : 1n, "int");
        }
        const scalar = coerceScalarResult(rhs, requireValue);
        if (isScalarError(scalar)) return scalar;
        if (node.op === "~") {
          if (isFloatingBase(scalar.base)) return makeCompileError();
          const promotedBase = integerPromotionBase(scalar.base);
          const value = normalizeIntegerForBase(
            scalar.value as bigint,
            promotedBase,
          );
          if (value == null) return makeCompileError();
          const bits = toUnsignedBits(value, promotedBase);
          const width = bitWidthForBase(promotedBase);
          if (!width) return makeCompileError();
          const mask = (1n << BigInt(width)) - 1n;
          const out = normalizeIntegerForBase((~bits) & mask, promotedBase);
          if (out == null) return makeCompileError();
          return makeRvalue(out, promotedBase);
        }
        if (node.op === "+")
          return makeRvalue(
            scalar.value!,
            isIntegerBase(scalar.base) ? integerPromotionBase(scalar.base) : scalar.base,
            0,
            "",
            scalar.nanSign,
          );
        if (node.op === "-") {
          if (isFloatingBase(scalar.base) && Number.isNaN(scalar.value)) {
            const flipped = scalar.nanSign === -1 ? 1 : -1;
            return makeRvalue(scalar.value!, scalar.base, 0, "", flipped);
          }
          if (!isFloatingBase(scalar.base)) {
            const promotedBase = integerPromotionBase(scalar.base);
            const value = normalizeIntegerForBase(
              scalar.value as bigint,
              promotedBase,
            );
            if (value == null) return makeCompileError();
            const range = integerRangeForBase(promotedBase);
            if (range && value === range.min) {
              return integerOverflowError(promotedBase);
            }
            const neg = normalizeIntegerForBase(-value, promotedBase);
            if (neg == null) return makeCompileError();
            return makeRvalue(neg, promotedBase);
          }
          return makeRvalue(-scalar.value!, scalar.base);
        }
        return makeCompileError();
      }
      if (node.kind === "binary") {
        if (node.op === "&&" || node.op === "||") {
          const left = evalNode(node.left);
          if (isEvalError(left)) return left;
          const leftRuntime = runtimeFromEval(left, true);
          if (isEvalError(leftRuntime)) return leftRuntime;
          const leftTruthy = runtimeTruthy(leftRuntime);
          if (node.op === "&&" && !leftTruthy) return makeRvalue(0n, "int");
          if (node.op === "||" && leftTruthy) return makeRvalue(1n, "int");
          const right = evalNode(node.right);
          if (isEvalError(right)) return right;
          const rightRuntime = runtimeFromEval(right, true);
          if (isEvalError(rightRuntime)) return rightRuntime;
          return makeRvalue(runtimeTruthy(rightRuntime) ? 1n : 0n, "int");
        }
        const left = evalNode(node.left);
        if (isEvalError(left)) return left;
        const right = evalNode(node.right);
        if (isEvalError(right)) return right;
        return evaluateBinaryResolved(node.op, left, right);
      }
      if (node.kind === "assign") {
        const lhs = evalNode(node.left);
        if (isEvalError(lhs)) return lhs;
        if (lhs.kind !== "lvalue") {
          return makeCompileError();
        }
        if (node.op === "=") {
          const rhs = evalNode(node.right);
          if (isEvalError(rhs)) return rhs;
          return assignIntoTarget(lhs, rhs);
        }
        const rhs = evalNode(node.right);
        if (isEvalError(rhs)) return rhs;
        const binaryOp = (() => {
          if (node.op === "+=") return "+";
          if (node.op === "-=") return "-";
          if (node.op === "*=") return "*";
          if (node.op === "/=") return "/";
          if (node.op === "%=") return "%";
          if (node.op === "<<=") return "<<";
          if (node.op === ">>=") return ">>";
          if (node.op === "&=") return "&";
          if (node.op === "^=") return "^";
          return "|";
        })() as BinaryOp;
        const lhsValue = runtimeFromEval(lhs, true);
        if (isEvalError(lhsValue)) return lhsValue;
        const lhsAsRvalue = makeRvalue(
          lhsValue.value,
          lhsValue.base,
          lhsValue.depth,
          lhsValue.label,
          lhsValue.nanSign,
          lhsValue.pointeeArrayDims,
        );
        const combined = evaluateBinaryResolved(binaryOp, lhsAsRvalue, rhs);
        if (isEvalError(combined)) return combined;
        return assignIntoTarget(lhs, combined);
      }
      return makeCompileError();
    }

    return evalNode(expr);
  }

  function evaluateExpression(
    expr: ExprNode,
    state: BoxState[],
    opts: {
      requireValue?: boolean;
    } = {},
  ): ScalarResult {
    const { requireValue = requireSourceValue } = opts;
    const evaluated = evaluateExpressionRaw(expr, state, opts);
    if (isEvalError(evaluated)) return evaluated;
    const scalar = coerceScalarResult(evaluated, requireValue);
    if (isScalarError(scalar)) return scalar;
    return scalar;
  }

  function evaluateCondition(
    expr: ExprNode,
    state: BoxState[],
  ): ConditionResult {
    const rawEvaluated = evaluateExpressionRaw(expr, state);
    if (isEvalError(rawEvaluated)) return rawEvaluated;
    const evaluated = decayArrayValue(rawEvaluated);
    if (isEvalError(evaluated)) return evaluated;
    if (requireSourceValue && String(evaluated.value ?? "") === "")
      return makeUbError();
    if (evaluated.depth > 0) {
      try {
        const addr = BigInt(String(evaluated.value ?? "").trim() || "0");
        return { value: addr !== 0n };
      } catch {
        return makeCompileError();
      }
    }
    const base = evaluated.base || "int";
    if (isFloatingBase(base)) {
      const parsed = parseDoubleValueWithSign(evaluated.value);
      if (!parsed) return makeCompileError();
      return { value: parsed.value !== 0 };
    }
    try {
      const intVal =
        typeof evaluated.value === "bigint"
          ? evaluated.value
          : BigInt(String(evaluated.value ?? "").trim() || "0");
      return { value: intVal !== 0n };
    } catch {
      return makeCompileError();
    }
  }

  function prependArrayDimension(typeText: string, length: number): string {
    const dim = Math.max(0, Math.floor(Number(length)));
    const clean = String(typeText || "").trim() || "int";
    const parsed = parseType(clean);
    if (
      parsed.base &&
      parsed.arrayDims?.length
    ) {
      const dims = [dim, ...parsed.arrayDims]
        .map((value) => `[${value}]`)
        .join("");
      return `${parsed.base}${"*".repeat(Math.max(0, Math.floor(parsed.depth)))}${dims}`;
    }
    const ptrArrayMatch = /^(.+?)\(\s*(\*+)\s*\)\s*((?:\[\s*\d+\s*\]\s*)+)\s*$/.exec(
      clean,
    );
    if (ptrArrayMatch) {
      const left = String(ptrArrayMatch[1] || "").trimEnd();
      const stars = String(ptrArrayMatch[2] || "");
      const dims = String(ptrArrayMatch[3] || "").replace(/\s+/g, "");
      return `${left} (${stars}[${dim}])${dims}`;
    }
    return `${clean}[${dim}]`;
  }

  function arrayExpressionType(result: EvalValue): string | null {
    if (!result.isArray) return null;
    const shape = normalizeArrayDims(result.arrayShape);
    if (!shape.length) return null;
    const depth = Math.floor(result.depth);
    const pointeeArrayDims = normalizeArrayDims(result.pointeeArrayDims);
    const pointeeInnerDepth = normalizePointeeInnerDepth(
      result.pointeeInnerDepth,
      depth,
      pointeeArrayDims,
    );
    const elementType =
      makePointerType(
        Math.max(0, depth - 1),
        result.base || "int",
        pointeeArrayDims,
        pointeeInnerDepth,
      ) ||
      `${result.base || "int"}${"*".repeat(Math.max(0, depth - 1))}`;
    return prependArrayDimension(elementType, shape[0]!);
  }

  function evaluateExpressionText(
    expr: string,
    state: BoxState[],
    opts: { allowSideEffects?: boolean } = {},
  ):
    | {
        result: {
          kind: string;
          type: string;
          value: BoxValue | bigint | number;
          address: string;
          nanSign?: -1 | 1;
        };
      }
    | {
        error: true;
        kind: "compile" | "ub";
      } {
    const { allowSideEffects = true } = opts;
    const tokens = tokenizeProgram(expr || "");
    if (!tokens.length) return makeCompileError();
    if (tokens.some((t) => t.type === "unknown")) {
      return makeCompileError();
    }
    if (tokens.some((t) => t.type === "sym" && t.value === ";")) {
      return makeCompileError();
    }
    const parsed = parseExpressionTokens(tokens, 0, { allowVars: true });
    if (!parsed || parsed.nextIndex !== tokens.length) {
      return makeCompileError();
    }
    let sawSideEffect = false;
    const evalState = allowSideEffects ? state : cloneBoxes(state);
    const rawEvaluated = evaluateExpressionRaw(parsed.expr, evalState, {
      onSideEffect: () => {
        sawSideEffect = true;
      },
    });
    if (isEvalError(rawEvaluated)) return rawEvaluated;
    if (!allowSideEffects && sawSideEffect) return makeCompileError();
    const evaluated = rawEvaluated;
    const resultType = evaluated.isArray
      ? arrayExpressionType(evaluated) ||
        makePointerType(
          evaluated.depth,
          evaluated.base || "int",
          normalizeArrayDims(evaluated.pointeeArrayDims),
          evaluated.pointeeInnerDepth,
        )
      : makePointerType(
          evaluated.depth,
          evaluated.base || "int",
          normalizeArrayDims(evaluated.pointeeArrayDims),
          evaluated.pointeeInnerDepth,
        );
    const resultKind = evaluated.isArray ? "rvalue" : evaluated.kind || "rvalue";
    const resultAddress =
      !evaluated.isArray && evaluated.kind === "lvalue" ? evaluated.address : "";
    return {
      result: {
        kind: resultKind,
        type: resultType || "int",
        value: evaluated.value,
        address: resultAddress,
        nanSign: evaluated.nanSign,
      },
    };
  }

  function convertScalarForAssignment(
    value: bigint | number,
    base: string,
    targetType: string,
    nanSign?: -1 | 1,
  ):
    | { value: bigint | number; base: string; nanSign?: -1 | 1 }
    | EvalError
    | null {
    const { base: targetBase, depth } = parseType(targetType);
    if (!targetBase || depth !== 0) return null;
    if (isFloatingBase(targetBase)) {
      const num =
        isFloatingBase(base)
          ? typeof value === "bigint"
            ? Number(value)
            : value
          : Number(value);
      const cast = targetBase === "float" ? Math.fround(num) : num;
      const nextNanSign = Number.isNaN(cast) ? (nanSign ?? 1) : undefined;
      return { value: cast, base: targetBase, nanSign: nextNanSign };
    }
    if (!isIntegerBase(targetBase)) return null;
    if (targetBase === "bool") {
      if (isFloatingBase(base)) {
        const num = typeof value === "bigint" ? Number(value) : value;
        return { value: num === 0 ? 0n : 1n, base: targetBase };
      }
      return { value: (value as bigint) === 0n ? 0n : 1n, base: targetBase };
    }
    let intValue: bigint;
    if (isFloatingBase(base)) {
      const num = typeof value === "bigint" ? Number(value) : value;
      if (!Number.isFinite(num)) return integerOverflowError(targetBase);
      try {
        intValue = BigInt(Math.trunc(num));
      } catch {
        return integerOverflowError(targetBase);
      }
      const range = integerRangeForBase(targetBase);
      if (range && (intValue < range.min || intValue > range.max)) {
        return integerOverflowError(targetBase);
      }
    } else {
      intValue = typeof value === "bigint" ? value : BigInt(Math.trunc(value));
    }
    const wrapped = wrapIntegerToBase(intValue, targetBase);
    if (wrapped == null) return null;
    return { value: wrapped, base: targetBase };
  }

  function convertAssignmentValue(
    evaluated: EvalResult,
    targetType: string,
    requireValue: boolean,
  ):
    | { value: string; nanSign?: -1 | 1 }
    | EvalError
    | { kind: "type-mismatch"; expectedType: string } {
    const source = decayArrayValue(evaluated);
    if (isEvalError(source)) return source;
    const {
      base: targetBase,
      depth: targetDepth,
      pointeeArrayDims: targetPointeeArrayDimsRaw,
      pointeeInnerDepth: targetPointeeInnerDepthRaw,
    } = parseType(targetType);
    if (!targetBase) {
      return makeCompileError();
    }
    const targetPointeeArrayDims = normalizeArrayDims(targetPointeeArrayDimsRaw);
    const targetPointeeInnerDepth = normalizePointeeInnerDepth(
      targetPointeeInnerDepthRaw,
      targetDepth,
      targetPointeeArrayDims,
    );
    if (targetDepth === 0) {
      const scalar = coerceScalarResult(source, requireValue);
      if (isScalarError(scalar)) return scalar;
      const converted = convertScalarForAssignment(
        scalar.value,
        scalar.base,
        targetType,
        scalar.nanSign,
      );
      if (!converted) return makeCompileError();
      if ("error" in converted) return converted;
      return {
        value: formatValueForType(converted.value, targetType, {
          nanSign: converted.nanSign,
        }),
        nanSign: converted.nanSign,
      };
    }
    const evalDepth = source.depth;
    const evalBase = source.base || "int";
    const evalPointeeArrayDims = normalizeArrayDims(source.pointeeArrayDims);
    const evalPointeeInnerDepth = normalizePointeeInnerDepth(
      source.pointeeInnerDepth,
      evalDepth,
      evalPointeeArrayDims,
    );
    if (evalDepth === 0 && isIntegerBase(evalBase)) {
      try {
        const rawInt =
          typeof source.value === "bigint"
            ? source.value
            : BigInt(String(source.value ?? "").trim() || "0");
        if (rawInt === 0n) {
          return { value: "0", nanSign: source.nanSign };
        }
      } catch {
        return makeCompileError();
      }
    }
    if (
      evalDepth !== targetDepth ||
      evalBase !== targetBase ||
      !samePointerPointeeType(
        evalDepth,
        evalPointeeArrayDims,
        evalPointeeInnerDepth,
        targetDepth,
        targetPointeeArrayDims,
        targetPointeeInnerDepth,
      )
    ) {
      const expectedType =
        makePointerType(
          evalDepth,
          evalBase,
          evalPointeeArrayDims,
          evalPointeeInnerDepth,
        ) ||
        `int${"*".repeat(evalDepth)}`;
      return { kind: "type-mismatch", expectedType };
    }
    if (requireValue && String(source.value ?? "") === "") return makeUbError();
    return {
      value: formatValueForType(source.value, targetType, {
        nanSign: source.nanSign,
      }),
      nanSign: source.nanSign,
    };
  }

  function validateAssignmentExpr(
    state: BoxState[],
    targetType: string,
    expr: ExprNode,
  ): EvalError | null {
    const evaluated = evaluateExpressionRaw(expr, cloneBoxes(state));
    const converted = convertAssignmentValue(
      evaluated,
      targetType,
      requireSourceValue,
    );
    if ("kind" in converted && converted.kind === "type-mismatch") {
      void converted.expectedType;
      return makeCompileError();
    }
    if ("error" in converted) return converted;
    return null;
  }

  function applyAssignmentToTarget(
    boxes: BoxState[],
    target: BoxState,
    targetType: string,
    expr: ExprNode,
  ): BoxState[] | null {
    const evaluated = evaluateExpressionRaw(expr, boxes);
    const converted = convertAssignmentValue(
      evaluated,
      targetType,
      requireSourceValue,
    );
    if (
      !converted ||
      "error" in converted ||
      ("kind" in converted && converted.kind === "type-mismatch")
    )
      return null;
    target.value = converted.value;
    return boxes;
  }

  function resolveAssignmentTarget(
    state: BoxState[],
    lhs: ExprNode,
  ):
    | { target: BoxState; targetType: string }
    | EvalError {
    const evaluated = evaluateExpressionRaw(lhs, state);
    if (isEvalError(evaluated)) return evaluated;
    if (evaluated.kind !== "lvalue") {
      return makeCompileError();
    }
    const { base, depth } = evaluated;
    const targetType =
      makePointerType(
        depth,
        base || "int",
        normalizeArrayDims(evaluated.pointeeArrayDims),
        evaluated.pointeeInnerDepth,
      ) || "int";
    const target = state.find(
      (b) => (b.address ?? "") === (evaluated.address ?? ""),
    );
    if (!target) return makeCompileError();
    return { target, targetType };
  }

  function applyStatement(
    state: BoxState[],
    stmt: Statement,
    opts: { alloc?: (type?: string) => string; allowRedeclare?: boolean },
  ): BoxState[] | null {
    if (!stmt) return state;
    const {
      alloc = (type) => String(randAddr(type || "int")),
      allowRedeclare = true,
    } = opts;
    const boxes = cloneBoxes(state);
    const by = Object.fromEntries(boxes.map((b) => [b.name, b]));
    if (stmt.kind === "decl") {
      const type = stmt.type || "int";
      if (stmt.arrayShape?.length) {
        const shape = stmt.arrayShape.map((d) => Math.max(0, Math.floor(Number(d))));
        if (!shape.length || shape.some((d) => d <= 0)) return null;
        let redeclare = false;
        forEachArrayIndex(shape, (indices) => {
          const elementName = arrayElementName(stmt.name, indices);
          if (by[elementName] && !allowRedeclare) redeclare = true;
          if (!by[elementName]) {
            boxes.push({
              name: elementName,
              type: stmt.elementType || type,
              value: "",
              address: alloc(stmt.elementType || type),
              arrayRoot: stmt.name,
              arrayShape: shape.slice(),
              arrayIndices: indices.slice(),
              pointeeArrayDims: stmt.elementPointeeArrayDims
                ? [...stmt.elementPointeeArrayDims]
                : [],
              pointeeInnerDepth: stmt.elementPointeeInnerDepth,
            });
          }
        });
        if (redeclare) return null;
      } else {
        if (by[stmt.name] && !allowRedeclare) return null;
        if (!by[stmt.name]) {
          boxes.push({
            name: stmt.name,
            type,
            value: "",
            address: alloc(type),
            pointeeArrayDims: stmt.pointeeArrayDims
              ? [...stmt.pointeeArrayDims]
              : [],
            pointeeInnerDepth: stmt.pointeeInnerDepth,
          });
        }
      }
      return boxes;
    }
    if (stmt.kind === "empty") {
      return boxes;
    }
    if (stmt.kind === "assign") {
      const evaluated = evaluateExpressionRaw(
        {
          kind: "assign",
          op: stmt.op,
          left: stmt.lhs,
          right: stmt.rhs,
        },
        boxes,
      );
      if (isEvalError(evaluated)) return null;
      return boxes;
    }
    if (stmt.kind === "declAssign") {
      const declType = stmt.declType || "int";
      if (by[stmt.name] && !allowRedeclare) return null;
      if (!by[stmt.name]) {
        boxes.push({
          name: stmt.name,
          type: declType,
          value: "",
          address: alloc(declType),
          pointeeArrayDims: stmt.pointeeArrayDims
            ? [...stmt.pointeeArrayDims]
            : [],
          pointeeInnerDepth: stmt.pointeeInnerDepth,
        });
      }
      const target = by[stmt.name] || boxes.find((b) => b.name === stmt.name);
      if (!target) return null;
      return applyAssignmentToTarget(boxes, target, declType, stmt.expr);
    }
    if (stmt.kind === "expr") {
      const evaluated = evaluateExpressionRaw(stmt.expr, boxes);
      if (isEvalError(evaluated)) return null;
      return boxes;
    }
    return null;
  }

  function hasArrayRoot(state: BoxState[], name: string): boolean {
    return state.some((box) => box.arrayRoot === name);
  }

  function findNamedBox(state: BoxState[], name: string): BoxState | null {
    return state.find((box) => box.name === name) || null;
  }

  function findFirstUninitializedRead(
    expr: ExprNode,
    state: BoxState[],
  ): string | null {
    const visit = (node: ExprNode, readContext: boolean): string | null => {
      if (node.kind === "num") return null;
      if (node.kind === "var") {
        if (!readContext) return null;
        if (hasArrayRoot(state, node.name)) return null;
        const box = findNamedBox(state, node.name);
        return box && String(box.value ?? "") === "" ? node.name : null;
      }
      if (node.kind === "cast") return visit(node.expr, true);
      if (node.kind === "postfix") return visit(node.expr, true);
      if (node.kind === "unary") {
        if (node.op === "&") return visit(node.expr, false);
        return visit(node.expr, true);
      }
      if (node.kind === "subscript") {
        return visit(node.left, true) || visit(node.index, true);
      }
      if (node.kind === "binary") {
        return visit(node.left, true) || visit(node.right, true);
      }
      if (node.kind === "assign") {
        if (node.op === "=") {
          return visit(node.left, false) || visit(node.right, true);
        }
        return visit(node.left, true) || visit(node.right, true);
      }
      return null;
    };
    return visit(expr, true);
  }

  function findFirstUninitializedTargetDependency(
    expr: ExprNode,
    state: BoxState[],
    requiresCurrentValue: boolean,
  ): string | null {
    if (expr.kind === "var") {
      if (!requiresCurrentValue) return null;
      const box = findNamedBox(state, expr.name);
      return box && String(box.value ?? "") === "" ? expr.name : null;
    }
    if (expr.kind === "unary" && expr.op === "*") {
      return findFirstUninitializedRead(expr.expr, state);
    }
    if (expr.kind === "subscript") {
      return (
        findFirstUninitializedRead(expr.left, state) ||
        findFirstUninitializedRead(expr.index, state)
      );
    }
    return findFirstUninitializedRead(expr, state);
  }

  function exprIsAssignable(expr: ExprNode): boolean {
    return (
      expr.kind === "var" ||
      expr.kind === "subscript" ||
      (expr.kind === "unary" && expr.op === "*")
    );
  }

  function findTopLevelAssignmentOperatorIndex(tokens: Token[]): number {
    let parenDepth = 0;
    let bracketDepth = 0;
    for (let i = 0; i < tokens.length; i++) {
      const token = tokens[i]!;
      if (token.type !== "sym") continue;
      if (token.value === "(") {
        parenDepth++;
        continue;
      }
      if (token.value === ")") {
        parenDepth--;
        continue;
      }
      if (token.value === "[") {
        bracketDepth++;
        continue;
      }
      if (token.value === "]") {
        bracketDepth--;
        continue;
      }
      if (parenDepth !== 0 || bracketDepth !== 0) continue;
      if (
        token.value === "=" ||
        token.value === "+=" ||
        token.value === "-=" ||
        token.value === "*=" ||
        token.value === "/=" ||
        token.value === "%=" ||
        token.value === "<<=" ||
        token.value === ">>=" ||
        token.value === "&=" ||
        token.value === "^=" ||
        token.value === "|="
      ) {
        return i;
      }
    }
    return -1;
  }

  function findDivisionByZero(expr: ExprNode, state: BoxState[]): boolean {
    let found = false;
    walkExpr(expr, (node) => {
      if (node.kind === "binary" && (node.op === "/" || node.op === "%")) {
        const right = evaluateExpression(node.right, cloneBoxes(state), {
          requireValue: true,
        });
        if ("error" in right) return false;
        if (isFloatingBase(right.base)) {
          const numeric = typeof right.value === "bigint" ? Number(right.value) : right.value;
          if (numeric === 0) {
            found = true;
            return true;
          }
          return false;
        }
        if (right.value === 0n) {
          found = true;
          return true;
        }
      }
      if (
        node.kind === "assign" &&
        (node.op === "/=" || node.op === "%=")
      ) {
        const right = evaluateExpression(node.right, cloneBoxes(state), {
          requireValue: true,
        });
        if ("error" in right) return false;
        if (isFloatingBase(right.base)) {
          const numeric = typeof right.value === "bigint" ? Number(right.value) : right.value;
          if (numeric === 0) {
            found = true;
            return true;
          }
          return false;
        }
        if (right.value === 0n) {
          found = true;
          return true;
        }
      }
      return false;
    });
    return found;
  }

  function findInvalidDereference(expr: ExprNode, state: BoxState[]): ProgramDiagnostic | null {
    let issue: ProgramDiagnostic | null = null;
    const emptyLines = [""];
    walkExpr(expr, (node) => {
      if (node.kind !== "unary" || node.op !== "*") return false;
      const evaluated = evaluateExpressionRaw(node.expr, cloneBoxes(state));
      if (isEvalError(evaluated)) return false;
      const source = decayArrayValue(evaluated);
      if (isEvalError(source)) return false;
      if (source.depth < 1) {
        issue = {
          kind: "compile",
          message: "You can only dereference a pointer value.",
          range: {
            startLine: 0,
            startCol: 0,
            endLine: 0,
            endCol: 1,
          },
        };
        return true;
      }
      const pointerValue = String(source.value ?? "").trim();
      if (pointerValue === "") {
        issue = {
          kind: "ub",
          message: "This pointer does not point anywhere yet.",
          tip: "Assign the pointer a valid address before dereferencing it.",
          range: normalizeDiagnosticRange(
            { startLine: 0, startCol: 0, endLine: 0, endCol: 1 },
            emptyLines,
          ),
        };
        return true;
      }
      try {
        if (BigInt(pointerValue) === 0n) {
          issue = {
            kind: "ub",
            message: "This pointer is 0, so dereferencing it here is invalid.",
            tip: "Make sure the pointer holds a real address before using *.",
            range: normalizeDiagnosticRange(
              { startLine: 0, startCol: 0, endLine: 0, endCol: 1 },
              emptyLines,
            ),
          };
          return true;
        }
      } catch {
        issue = {
          kind: "compile",
          message: "This does not look like a valid pointer value.",
          range: normalizeDiagnosticRange(
            { startLine: 0, startCol: 0, endLine: 0, endCol: 1 },
            emptyLines,
          ),
        };
        return true;
      }
      return false;
    });
    return issue;
  }

  function expressionFailureDiagnostic(
    tokens: Token[],
    expr: ExprNode,
    state: BoxState[],
    declared: DeclaredNames,
    lines: string[],
    opts: { targetType?: string; statementRange?: ProgramDiagnosticRange } = {},
  ): ProgramDiagnostic | null {
    const {
      targetType = "double",
      statementRange = rangeFromTokens(tokens, lines),
    } = opts;
    const missing = findFirstUndeclaredVariable(expr, declared);
    if (missing) {
      const token = findIdentifierToken(tokens, missing);
      return makeDiagnostic(
        "compile",
        `I do not know what ${JSON.stringify(missing)} is yet.`,
        token ? rangeFromToken(token, lines) : statementRange,
        lines,
        "Declare the variable before you use it.",
      );
    }
    const uninitialized = findFirstUninitializedRead(expr, state);
    if (uninitialized) {
      const token = findIdentifierToken(tokens, uninitialized);
      return makeDiagnostic(
        "ub",
        `${JSON.stringify(uninitialized)} exists, but it does not have a value yet.`,
        token ? rangeFromToken(token, lines) : statementRange,
        lines,
        "Assign to it before you read from it.",
      );
    }
    if (findDivisionByZero(expr, state)) {
      return makeDiagnostic(
        "ub",
        "This divides by 0, which is undefined.",
        statementRange,
        lines,
        "Make sure the value on the right side is not zero.",
      );
    }
    const dereferenceIssue = findInvalidDereference(expr, state);
    if (dereferenceIssue) {
      return makeDiagnostic(
        dereferenceIssue.kind,
        dereferenceIssue.message,
        statementRange,
        lines,
        dereferenceIssue.tip,
      );
    }
    const evaluated = evaluateExpressionRaw(expr, cloneBoxes(state));
    const converted = convertAssignmentValue(evaluated, targetType, requireSourceValue);
    if ("kind" in converted && converted.kind === "type-mismatch") {
      return makeDiagnostic(
        "compile",
        `This expression has type ${converted.expectedType}, but the target expects ${targetType}.`,
        statementRange,
        lines,
        "The types on both sides need to match here.",
      );
    }
    if ("error" in converted) {
      return makeDiagnostic(
        converted.kind,
        converted.kind === "ub"
          ? "This expression reaches undefined behavior with the current values."
          : "This expression does not work with the current variable types.",
        statementRange,
        lines,
        converted.kind === "ub"
          ? "Common causes are uninitialized values, invalid pointers, or dividing by 0."
          : "Double-check the operators and the types of the values you are combining.",
      );
    }
    if (isEvalError(evaluated)) {
      return makeDiagnostic(
        evaluated.kind,
        evaluated.kind === "ub"
          ? "This expression reaches undefined behavior with the current values."
          : "This expression does not work with the current variable types.",
        statementRange,
        lines,
      );
    }
    return null;
  }

  function diagnoseWritableTarget(
    tokens: Token[],
    lhs: ExprNode,
    state: BoxState[],
    declared: DeclaredNames,
    lines: string[],
    requiresCurrentValue: boolean,
  ):
    | { target: BoxState; targetType: string }
    | { diagnostic: ProgramDiagnostic } {
    const targetRange = rangeFromTokens(tokens, lines);
    const missing = findFirstUndeclaredVariable(lhs, declared);
    if (missing) {
      const token = findIdentifierToken(tokens, missing);
      return {
        diagnostic: makeDiagnostic(
          "compile",
          `I do not know what ${JSON.stringify(missing)} is yet.`,
          token ? rangeFromToken(token, lines) : targetRange,
          lines,
          "Declare the variable before you use it.",
        ),
      };
    }
    const uninitialized = findFirstUninitializedTargetDependency(
      lhs,
      state,
      requiresCurrentValue,
    );
    if (uninitialized) {
      const token = findIdentifierToken(tokens, uninitialized);
      return {
        diagnostic: makeDiagnostic(
          "ub",
          `${JSON.stringify(uninitialized)} does not have a usable value yet.`,
          token ? rangeFromToken(token, lines) : targetRange,
          lines,
          "Initialize it before you use it as part of an assignment target.",
        ),
      };
    }
    if (!exprIsAssignable(lhs)) {
      return {
        diagnostic: makeDiagnostic(
          "compile",
          "The left side of this assignment must be something you can store into.",
          targetRange,
          lines,
          "Use a variable, an array element, or *pointer on the left side.",
        ),
      };
    }
    const resolved = resolveAssignmentTarget(cloneBoxes(state), lhs);
    if ("error" in resolved) {
      return {
        diagnostic: makeDiagnostic(
          resolved.kind,
          resolved.kind === "ub"
            ? "This assignment target is invalid with the current values."
            : "I cannot assign into this target.",
          targetRange,
          lines,
        ),
      };
    }
    return resolved;
  }

  function validateStatement(
    tokens: Token[],
    state: BoxState[],
    seenDecl: DeclaredNames,
    alloc: (type?: string) => string,
  ): StatementValidationResult {
    if (tokens.some((t) => t.type === "unknown")) return makeCompileError();
    const parsed = parseStatementTokens(tokens);
    if (!parsed) return makeCompileError();
    if (parsed.kind === "blockStart" || parsed.kind === "blockEnd") {
      return { parsed, next: state };
    }
    if (parsed.kind === "else") {
      return { parsed, next: state };
    }
    if (parsed.kind === "if") {
      const result = evaluateCondition(parsed.expr, cloneBoxes(state));
      if ("error" in result) return result;
      return { parsed, next: state };
    }
    if (parsed.kind === "while") {
      const result = evaluateCondition(parsed.expr, cloneBoxes(state));
      if ("error" in result) return result;
      return { parsed, next: state };
    }
    if (parsed.kind === "decl" || parsed.kind === "declAssign") {
      if (seenDecl.has(parsed.name)) return makeCompileError();
      if (parsed.kind === "declAssign") {
        const targetType = parsed.declType || "int";
        const err = validateAssignmentExpr(state, targetType, parsed.expr);
        if (err) return err;
      }
    } else if (parsed.kind === "assign") {
      if (parsed.op === "=") {
        const resolved = resolveAssignmentTarget(state, parsed.lhs);
        if ("error" in resolved) return resolved;
        const err = validateAssignmentExpr(
          state,
          resolved.target.type,
          parsed.rhs,
        );
        if (err) return err;
      }
      const checked = evaluateExpressionRaw(
        {
          kind: "assign",
          op: parsed.op,
          left: parsed.lhs,
          right: parsed.rhs,
        },
        cloneBoxes(state),
      );
      if (isEvalError(checked)) return checked;
    } else if (parsed.kind === "expr") {
      const checked = evaluateExpressionRaw(parsed.expr, cloneBoxes(state));
      if (isEvalError(checked)) return checked;
    }
    const next = applyStatement(state, parsed, {
      alloc,
      allowRedeclare: false,
    });
    if (!next) return makeCompileError();
    return { next, parsed };
  }

  function isBracePart(part: StatementPart, brace?: "{" | "}") {
    if (part.tokens.length !== 1) return false;
    const tok = part.tokens[0];
    if (tok.type !== "sym") return false;
    if (brace) return tok.value === brace;
    return tok.value === "{" || tok.value === "}";
  }

  function isElsePart(part: StatementPart | undefined): boolean {
    if (!part || part.tokens.length !== 1) return false;
    const tok = part.tokens[0];
    return tok.type === "kw" && tok.value === "else";
  }

  function isDeclarationPart(part: StatementPart | undefined): boolean {
    if (!part || !part.tokens.length) return false;
    const parsed = parseStatementTokens(part.tokens);
    return parsed?.kind === "decl" || parsed?.kind === "declAssign";
  }

  function parseControlStatementMaps(
    parts: StatementPart[],
    opts: { lastLine?: number } = {},
  ): {
    ifMap: Map<number, IfBlock>;
    whileMap: Map<number, WhileBlock>;
  } {
    const ifMap = new Map<number, IfBlock>();
    const whileMap = new Map<number, WhileBlock>();
    const fallbackLastLine =
      parts.length > 0 ? parts[parts.length - 1]!.endLine : 0;
    const lastLine = Math.max(0, opts.lastLine ?? fallbackLastLine);
    type StatementExtent =
      | { kind: "ok"; endIndex: number }
      | { kind: "error" }
      | { kind: "incomplete"; line: number };
    const extentMemo = new Map<number, StatementExtent>();
    const ifMemo = new Map<number, StatementExtent>();
    const whileMemo = new Map<number, StatementExtent>();
    function parseStatementExtent(startIndex: number): StatementExtent {
      if (startIndex >= parts.length) {
        return { kind: "incomplete", line: lastLine };
      }
      const memoized = extentMemo.get(startIndex);
      if (memoized) return memoized;
      const part = parts[startIndex];
      if (!part.tokens.length) {
        const result: StatementExtent = { kind: "ok", endIndex: startIndex };
        extentMemo.set(startIndex, result);
        return result;
      }
      if (isElsePart(part)) {
        const result: StatementExtent = { kind: "error" };
        extentMemo.set(startIndex, result);
        return result;
      }
      const ifParsed = parseIfHeaderTokens(part.tokens);
      if (ifParsed) {
        const result = parseIfAt(startIndex, ifParsed);
        extentMemo.set(startIndex, result);
        return result;
      }
      const whileParsed = parseWhileHeaderTokens(part.tokens);
      if (whileParsed) {
        const result = parseWhileAt(startIndex, whileParsed);
        extentMemo.set(startIndex, result);
        return result;
      }
      if (isBracePart(part, "{")) {
        let depth = 0;
        for (let i = startIndex; i < parts.length; i++) {
          const probe = parts[i];
          if (isBracePart(probe, "{")) {
            depth++;
            continue;
          }
          if (isBracePart(probe, "}")) {
            depth--;
            if (depth === 0) {
              const result: StatementExtent = { kind: "ok", endIndex: i };
              extentMemo.set(startIndex, result);
              return result;
            }
          }
        }
        const result: StatementExtent = { kind: "incomplete", line: lastLine };
        extentMemo.set(startIndex, result);
        return result;
      }
      const result: StatementExtent = { kind: "ok", endIndex: startIndex };
      extentMemo.set(startIndex, result);
      return result;
    }

    function parseIfAt(
      headerIndex: number,
      ifParsed: { expr: ExprNode; hasVar: boolean },
    ): StatementExtent {
      const memoized = ifMemo.get(headerIndex);
      if (memoized) return memoized;
      const header = parts[headerIndex];
      const headerStartLine = header.startLine;
      const headerEndLine = header.endLine;
      const openIndex = headerIndex + 1;
      const openPart = parts[openIndex];
      if (!openPart) {
        const result: StatementExtent = { kind: "incomplete", line: lastLine };
        ifMemo.set(headerIndex, result);
        return result;
      }
      const thenUsesBraces = isBracePart(openPart, "{");
      if (!thenUsesBraces && isDeclarationPart(openPart)) {
        const result: StatementExtent = { kind: "error" };
        ifMemo.set(headerIndex, result);
        return result;
      }
      const thenExtent = parseStatementExtent(openIndex);
      if (thenExtent.kind !== "ok") {
        ifMemo.set(headerIndex, thenExtent);
        return thenExtent;
      }
      const closeIndex = thenExtent.endIndex;
      let trueTarget = openIndex;
      if (thenUsesBraces && closeIndex > openIndex + 1) {
        trueTarget = openIndex + 1;
      }
      let elseIndex: number | null = null;
      let elseOpenIndex: number | null = null;
      let elseCloseIndex: number | null = null;
      let elseTarget: number | null = null;
      let afterIndex = closeIndex + 1;
      const possibleElseIndex = closeIndex + 1;
      const possibleElse = parts[possibleElseIndex];
      if (isElsePart(possibleElse)) {
        elseIndex = possibleElseIndex;
        elseOpenIndex = possibleElseIndex + 1;
        const elseOpenPart = parts[elseOpenIndex];
        if (!elseOpenPart) {
          const result: StatementExtent = {
            kind: "incomplete",
            line: lastLine,
          };
          ifMemo.set(headerIndex, result);
          return result;
        }
        const elseUsesBraces = isBracePart(elseOpenPart, "{");
        if (!elseUsesBraces && isDeclarationPart(elseOpenPart)) {
          const result: StatementExtent = { kind: "error" };
          ifMemo.set(headerIndex, result);
          return result;
        }
        const elseExtent = parseStatementExtent(elseOpenIndex);
        if (elseExtent.kind !== "ok") {
          ifMemo.set(headerIndex, elseExtent);
          return elseExtent;
        }
        elseCloseIndex = elseExtent.endIndex;
        afterIndex = elseCloseIndex + 1;
        elseTarget = elseOpenIndex;
        if (elseUsesBraces && elseCloseIndex >= elseOpenIndex + 1) {
          elseTarget = elseOpenIndex + 1;
        }
      }
      const falseTarget =
        elseTarget ??
        (afterIndex < parts.length ? afterIndex : parts.length);
      ifMap.set(headerIndex, {
        headerIndex,
        headerStartLine,
        headerEndLine,
        openIndex,
        closeIndex,
        trueTarget,
        falseTarget,
        elseIndex,
        elseOpenIndex,
        elseCloseIndex,
        elseTarget,
        afterIndex,
        expr: ifParsed.expr,
        hasVar: ifParsed.hasVar,
      });
      const result: StatementExtent = {
        kind: "ok",
        endIndex: elseCloseIndex ?? closeIndex,
      };
      ifMemo.set(headerIndex, result);
      return result;
    }

    function parseWhileAt(
      headerIndex: number,
      whileParsed: { expr: ExprNode; hasVar: boolean },
    ): StatementExtent {
      const memoized = whileMemo.get(headerIndex);
      if (memoized) return memoized;
      const header = parts[headerIndex];
      const headerStartLine = header.startLine;
      const headerEndLine = header.endLine;
      const openIndex = headerIndex + 1;
      const openPart = parts[openIndex];
      if (!openPart) {
        const result: StatementExtent = { kind: "incomplete", line: lastLine };
        whileMemo.set(headerIndex, result);
        return result;
      }
      const bodyUsesBraces = isBracePart(openPart, "{");
      if (!bodyUsesBraces && isDeclarationPart(openPart)) {
        const result: StatementExtent = { kind: "error" };
        whileMemo.set(headerIndex, result);
        return result;
      }
      const bodyExtent = parseStatementExtent(openIndex);
      if (bodyExtent.kind !== "ok") {
        whileMemo.set(headerIndex, bodyExtent);
        return bodyExtent;
      }
      const closeIndex = bodyExtent.endIndex;
      let trueTarget = openIndex;
      if (bodyUsesBraces && closeIndex > openIndex + 1) {
        trueTarget = openIndex + 1;
      }
      const afterIndex = closeIndex + 1;
      whileMap.set(headerIndex, {
        headerIndex,
        headerStartLine,
        headerEndLine,
        openIndex,
        closeIndex,
        trueTarget,
        afterIndex,
        expr: whileParsed.expr,
        hasVar: whileParsed.hasVar,
      });
      const result: StatementExtent = { kind: "ok", endIndex: closeIndex };
      whileMemo.set(headerIndex, result);
      return result;
    }

    for (let i = 0; i < parts.length; i++) {
      const part = parts[i];
      if (!part.tokens.length) continue;
      const ifParsed = parseIfHeaderTokens(part.tokens);
      if (ifParsed) {
        parseIfAt(i, ifParsed);
        continue;
      }
      const whileParsed = parseWhileHeaderTokens(part.tokens);
      if (!whileParsed) continue;
      parseWhileAt(i, whileParsed);
    }
    return { ifMap, whileMap };
  }

  function buildIfStatementMap(
    parts: StatementPart[],
    opts: { lastLine?: number } = {},
  ): IfBlockMap {
    const parsed = parseControlStatementMaps(parts, opts);
    return {
      map: parsed.ifMap,
    };
  }

  function buildWhileStatementMap(
    parts: StatementPart[],
    opts: { lastLine?: number } = {},
  ): WhileBlockMap {
    const parsed = parseControlStatementMaps(parts, opts);
    return {
      map: parsed.whileMap,
    };
  }

  function buildStatementMap(lines: string[]): StatementMap {
    const text = lines.join("\n");
    const tokens = tokenizeProgram(text);
    const parts = splitStatements(tokens);
    const byLine: Array<StatementRange | null> = new Array(lines.length).fill(
      null,
    );
    parts.forEach((part) => {
      const range: StatementRange = {
        startLine: part.startLine,
        endLine: part.endLine,
        hasSemicolon: !!part.hasSemicolon,
      };
      for (let i = range.startLine; i <= range.endLine; i++) {
        if (i >= 0 && i < byLine.length && !byLine[i]) byLine[i] = range;
      }
    });
    return { parts, byLine };
  }

  function statementRangeForLine(
    statementMap: StatementMap,
    lineIndex: number,
  ): StatementRange | null {
    if (lineIndex < 0 || lineIndex >= statementMap.byLine.length) return null;
    return statementMap.byLine[lineIndex];
  }

  function getStatementContext(
    lines: string[],
    boundary: number,
  ): StatementContext {
    const statementMap = buildStatementMap(lines);
    const currentRange = statementRangeForLine(statementMap, boundary);
    const prevRange = statementRangeForLine(statementMap, boundary - 1);
    const isMultiLine =
      currentRange !== null && currentRange.endLine > currentRange.startLine;
    const midStatement =
      isMultiLine &&
      boundary > currentRange.startLine &&
      boundary <= currentRange.endLine;
    const atStatementStart = isMultiLine && boundary === currentRange.startLine;
    return {
      statementMap,
      currentRange,
      prevRange,
      midStatement,
      atStatementStart,
    };
  }

  function findMissingSemicolonLines(text: string): number[] {
    const lines = text.split(/\r?\n/);
    const missing: number[] = [];
    const patched: string[] = [];
    let inBlock = false;
    lines.forEach((line, idx) => {
      const raw = line;
      let i = 0;
      let lastCodeIndex = -1;
      let sawCode = false;
      while (i < raw.length) {
        const ch = raw[i];
        const next = raw[i + 1];
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
        if (ch === "{" || ch === "}") {
          i += 1;
          continue;
        }
        if (!/\s/.test(ch)) {
          sawCode = true;
          lastCodeIndex = i;
        }
        i += 1;
      }
      if (!sawCode) {
        patched.push(raw);
        return;
      }
      const clean = stripAllComments(raw).trim();
      if (!clean || clean === "{" || clean === "}") {
        patched.push(raw);
        return;
      }
      if (/^if\b/.test(clean)) {
        patched.push(raw);
        return;
      }
      if (/^while\b/.test(clean)) {
        patched.push(raw);
        return;
      }
      if (/^}\s*else\b/.test(clean)) {
        patched.push(raw);
        return;
      }
      if (/^else\b/.test(clean)) {
        patched.push(raw);
        return;
      }
      if (raw[lastCodeIndex] === ";") {
        patched.push(raw);
        return;
      }
      missing.push(idx + 1);
      patched.push(
        `${raw.slice(0, lastCodeIndex + 1)};${raw.slice(lastCodeIndex + 1)}`,
      );
    });
    if (!missing.length) return [];
    const patchedText = patched.join("\n");
    const state = applyProgram(patchedText);
    if (!state) return [];
    return missing;
  }

  function parseStatements(text: string): Statement[] {
    const tokens = tokenizeProgram(text);
    const parts = splitStatements(tokens);
    const statements: Statement[] = [];
    for (const part of parts) {
      if (!part.tokens.length) continue;
      const parsed = parseStatementTokens(part.tokens);
      if (!parsed) continue;
      if (part.hasSemicolon) statements.push(parsed);
    }
    return statements;
  }

  function executeProgramParts(
    parts: StatementPart[],
    opts: {
      alloc: (type?: string) => string;
      stop: number | null;
      stopSteps: number | null;
      analyze: boolean;
    },
  ):
    | {
        kind: "ok";
        state: BoxState[];
        nextIndex: number;
        executedSteps: number;
      }
    | { kind: "compile" | "ub" } {
    let state: BoxState[] = [];
    const declared = new Set<string>();
    const scopes: ScopeStack = [new Set<string>()];
    const ifBlocks = buildIfStatementMap(parts);
    const whileBlocks = buildWhileStatementMap(parts);
    const ifDecisions = new Map<number, boolean>();
    const elseLookup = new Map<number, IfBlock>();
    const whileStack: WhileBlock[] = [];
    ifBlocks.map.forEach((block) => {
      if (block.elseIndex != null) {
        elseLookup.set(block.elseIndex, block);
      }
    });
    const continueCompletedWhileLoops = (nextIndex: number): number => {
      let index = nextIndex;
      let guard = 0;
      while (whileStack.length > 0 && guard < parts.length + 5) {
        const active = whileStack[whileStack.length - 1];
        if (!active || index !== active.afterIndex) break;
        whileStack.pop();
        index = active.headerIndex;
        guard += 1;
      }
      return index;
    };
    const elseEntryForIfBlock = (
      block: IfBlock,
    ): { nextIndex: number; pushScope: boolean } | null => {
      if (block.elseOpenIndex == null) return null;
      let nextIndex = block.elseOpenIndex;
      let pushScope = false;
      if (block.elseTarget != null && block.elseTarget !== block.elseOpenIndex) {
        const elseOpenPart = parts[block.elseOpenIndex];
        if (isBracePart(elseOpenPart, "{")) {
          nextIndex = block.elseTarget;
          pushScope = true;
        }
      }
      return { nextIndex, pushScope };
    };
    const maxExecutedParts = Math.max(10000, parts.length * 2000);
    const fallbackEndLine =
      parts.length > 0 ? parts[parts.length - 1]!.endLine : -1;
    const terminalBoundary = Math.max(0, fallbackEndLine + 1);
    const boundaryForProgramIndex = (index: number): number => {
      if (index >= parts.length) return terminalBoundary;
      const safeIndex = Math.max(0, Math.floor(index));
      return Math.max(0, parts[safeIndex]!.startLine);
    };
    let i = 0;
    let executedSteps = 0;
    let executedParts = 0;
    type ControlFlowResult =
      | { kind: "not-control" }
      | { kind: "continue" }
      | { kind: "break" }
      | { kind: "error"; errorKind: "compile" | "ub" };
    const isDeclLikeStatement = (
      parsed: Statement,
    ): parsed is Extract<Statement, { kind: "decl" | "declAssign" }> =>
      parsed.kind === "decl" || parsed.kind === "declAssign";
    const advanceTo = (nextIndex: number): number => {
      const before = boundaryForProgramIndex(i);
      const after = boundaryForProgramIndex(nextIndex);
      if (after !== before) executedSteps += 1;
      return nextIndex;
    };
    const handleControlFlowStatement = (
      parsed: Statement,
      blockEndState: BoxState[],
    ): ControlFlowResult => {
      if (parsed.kind === "if") {
        const block = ifBlocks.map.get(i);
        if (!block) return { kind: "error", errorKind: "compile" };
        const condition = evaluateCondition(parsed.expr, state);
        if ("error" in condition)
          return { kind: "error", errorKind: condition.kind || "compile" };
        ifDecisions.set(block.headerIndex, condition.value);
        if (condition.value) {
          i = advanceTo(i + 1);
          return { kind: "continue" };
        }
        if (opts.stop !== null && opts.stop <= block.closeIndex) {
          return { kind: "break" };
        }
        const elseEntry = elseEntryForIfBlock(block);
        if (elseEntry) {
          if (elseEntry.pushScope) scopes.push(new Set<string>());
          i = advanceTo(continueCompletedWhileLoops(elseEntry.nextIndex));
          return { kind: "continue" };
        }
        i = advanceTo(continueCompletedWhileLoops(block.closeIndex + 1));
        return { kind: "continue" };
      }
      if (parsed.kind === "while") {
        const block = whileBlocks.map.get(i);
        if (!block) return { kind: "error", errorKind: "compile" };
        const condition = evaluateCondition(parsed.expr, state);
        if ("error" in condition)
          return { kind: "error", errorKind: condition.kind || "compile" };
        if (condition.value) {
          whileStack.push(block);
          i = advanceTo(block.openIndex);
          return { kind: "continue" };
        }
        if (opts.stop !== null && opts.stop <= block.closeIndex) {
          return { kind: "break" };
        }
        i = advanceTo(continueCompletedWhileLoops(block.afterIndex));
        return { kind: "continue" };
      }
      if (parsed.kind === "else") {
        const block = elseLookup.get(i);
        if (!block) return { kind: "error", errorKind: "compile" };
        const decision = ifDecisions.get(block.headerIndex);
        if (decision == null) return { kind: "error", errorKind: "compile" };
        if (decision) {
          i = advanceTo(continueCompletedWhileLoops(block.afterIndex));
          return { kind: "continue" };
        }
        const elseEntry = elseEntryForIfBlock(block);
        if (elseEntry) {
          if (elseEntry.pushScope) scopes.push(new Set<string>());
          i = advanceTo(continueCompletedWhileLoops(elseEntry.nextIndex));
          return { kind: "continue" };
        }
        return { kind: "error", errorKind: "compile" };
      }
      if (parsed.kind === "blockStart") {
        scopes.push(new Set<string>());
        i = advanceTo(continueCompletedWhileLoops(i + 1));
        return { kind: "continue" };
      }
      if (parsed.kind === "blockEnd") {
        const popped = popScope(scopes, declared, blockEndState);
        if (popped.error) return { kind: "error", errorKind: "compile" };
        state = popped.state;
        i = advanceTo(continueCompletedWhileLoops(i + 1));
        return { kind: "continue" };
      }
      return { kind: "not-control" };
    };
    while (i < parts.length) {
      if (opts.stop !== null && i >= opts.stop) break;
      if (opts.stopSteps !== null && executedSteps >= opts.stopSteps) break;
      if (executedParts >= maxExecutedParts) {
        return { kind: "compile" };
      }
      executedParts += 1;
      const part = parts[i];
      if (!part.tokens.length) {
        i = continueCompletedWhileLoops(i + 1);
        continue;
      }

      if (opts.analyze) {
        const validation = validateStatement(part.tokens, state, declared, opts.alloc);
        if ("error" in validation) {
          return { kind: validation.kind || "compile" };
        }
        const parsed = validation.parsed;
        const controlResult = handleControlFlowStatement(parsed, validation.next);
        if (controlResult.kind === "error") {
          return { kind: controlResult.errorKind };
        }
        if (controlResult.kind === "break") break;
        if (controlResult.kind === "continue") continue;
        if (!part.hasSemicolon) return { kind: "compile" };
        if (isDeclLikeStatement(parsed)) {
          addDeclaredNames(scopes, declared, parsed.declaredNames);
        }
        state = validation.next;
        i = advanceTo(continueCompletedWhileLoops(i + 1));
        continue;
      }

      const parsed = parseStatementTokens(part.tokens);
      if (!parsed) return { kind: "compile" };
      const controlResult = handleControlFlowStatement(parsed, state);
      if (controlResult.kind === "error") return { kind: controlResult.errorKind };
      if (controlResult.kind === "break") break;
      if (controlResult.kind === "continue") continue;
      if (!part.hasSemicolon) return { kind: "compile" };
      if (isDeclLikeStatement(parsed)) {
        if (declared.has(parsed.name)) return { kind: "compile" };
      }
      const next = applyStatement(state, parsed, {
        alloc: opts.alloc,
        allowRedeclare: false,
      });
      if (!next) return { kind: "compile" };
      if (isDeclLikeStatement(parsed)) {
        addDeclaredNames(scopes, declared, parsed.declaredNames);
      }
      state = next;
      i = advanceTo(continueCompletedWhileLoops(i + 1));
    }
    return {
      kind: "ok",
      state,
      nextIndex: Math.max(0, Math.min(parts.length, i)),
      executedSteps,
    };
  }

  function applyProgram(
    text: string,
    opts: { alloc?: (type?: string) => string } = {},
  ): BoxState[] | null {
    const tokens = tokenizeProgram(text);
    const parts = splitStatements(tokens);
    return applyProgramParts(parts, opts);
  }

  const resolveAlloc = (alloc?: (type?: string) => string) =>
    alloc || ((type?: string) => String(randAddr(type || "int")));

  const normalizeStop = (stop: number | undefined, max: number): number | null =>
    stop === undefined ? null : Math.max(0, Math.min(max, stop));

  const normalizeStopSteps = (stopSteps: number | undefined): number | null =>
    stopSteps === undefined ? null : Math.max(0, stopSteps);

  function applyProgramParts(
    parts: StatementPart[],
    opts: { alloc?: (type?: string) => string; stop?: number } = {},
  ): BoxState[] | null {
    const alloc = resolveAlloc(opts.alloc);
    const stop = normalizeStop(opts.stop, parts.length);
    const result = executeProgramParts(parts, {
      alloc,
      stop,
      stopSteps: null,
      analyze: false,
    });
    if (result.kind !== "ok") return null;
    return result.state;
  }

  function analyzeProgramParts(
    parts: StatementPart[],
    opts: { alloc?: (type?: string) => string; stop?: number } = {},
  ): ProgramResult {
    const alloc = resolveAlloc(opts.alloc);
    const stop = normalizeStop(opts.stop, parts.length);
    const result = executeProgramParts(parts, {
      alloc,
      stop,
      stopSteps: null,
      analyze: true,
    });
    if (result.kind !== "ok") return { kind: result.kind };
    return { kind: "ok", state: result.state };
  }

  function traceProgramParts(
    parts: StatementPart[],
    opts: { alloc?: (type?: string) => string; stopSteps?: number } = {},
  ): ProgramTrace | null {
    const alloc = resolveAlloc(opts.alloc);
    const stopSteps = normalizeStopSteps(opts.stopSteps);
    const result = executeProgramParts(parts, {
      alloc,
      stop: null,
      stopSteps,
      analyze: false,
    });
    if (result.kind !== "ok") return null;
    return {
      state: result.state,
      nextIndex: result.nextIndex,
      executedSteps: result.executedSteps,
    };
  }

  function diagnoseProgram(
    text: string,
    opts: { alloc?: (type?: string) => string } = {},
  ): ProgramDiagnostic[] {
    const lines = text.split(/\r?\n/);
    const tokens = tokenizeProgram(text);
    const parts = splitStatements(tokens);
    const candidates: ProgramDiagnostic[] = [];
    for (const token of tokens) {
      if (token.type !== "unknown") continue;
      candidates.push(diagnoseUnknownToken(token, lines));
    }
    const unmatchedDelimiter = findUnmatchedDelimiterDiagnostic(tokens, lines);
    if (unmatchedDelimiter) candidates.push(unmatchedDelimiter);
    for (const part of parts) {
      if (!part.tokens.length) continue;
      const typoDiagnostic = diagnoseUnknownKeywordOrType(part.tokens, lines);
      if (typoDiagnostic) candidates.push(typoDiagnostic);
    }
    for (const lineNumber of findMissingSemicolonLines(text)) {
      candidates.push(missingSemicolonDiagnostic(lines, lineNumber));
    }
    if (candidates.length > 0) {
      candidates.sort(compareDiagnosticLocation);
      return [candidates[0]!];
    }
    const alloc = resolveAlloc(opts.alloc);
    const declared = new Set<string>();
    const scopes: ScopeStack = [new Set<string>()];
    let state: BoxState[] = [];
    const ifBlocks = buildIfStatementMap(parts, {
      lastLine: Math.max(0, lines.length - 1),
    });
    const whileBlocks = buildWhileStatementMap(parts, {
      lastLine: Math.max(0, lines.length - 1),
    });
    const ifDecisions = new Map<number, boolean>();
    const elseLookup = new Map<number, IfBlock>();
    const whileStack: WhileBlock[] = [];
    ifBlocks.map.forEach((block) => {
      if (block.elseIndex != null) {
        elseLookup.set(block.elseIndex, block);
      }
    });
    const continueCompletedWhileLoops = (nextIndex: number): number => {
      let index = nextIndex;
      let guard = 0;
      while (whileStack.length > 0 && guard < parts.length + 5) {
        const active = whileStack[whileStack.length - 1];
        if (!active || index !== active.afterIndex) break;
        whileStack.pop();
        index = active.headerIndex;
        guard += 1;
      }
      return index;
    };
    const elseEntryForIfBlock = (
      block: IfBlock,
    ): { nextIndex: number; pushScope: boolean } | null => {
      if (block.elseOpenIndex == null) return null;
      let nextIndex = block.elseOpenIndex;
      let pushScope = false;
      if (block.elseTarget != null && block.elseTarget !== block.elseOpenIndex) {
        const elseOpenPart = parts[block.elseOpenIndex];
        if (isBracePart(elseOpenPart, "{")) {
          nextIndex = block.elseTarget;
          pushScope = true;
        }
      }
      return { nextIndex, pushScope };
    };
    const isDeclLikeStatement = (
      parsed: Statement,
    ): parsed is Extract<Statement, { kind: "decl" | "declAssign" }> =>
      parsed.kind === "decl" || parsed.kind === "declAssign";
    const maxExecutedParts = Math.max(10000, parts.length * 2000);
    let executedParts = 0;
    let i = 0;

    while (i < parts.length) {
      if (executedParts >= maxExecutedParts) {
        const rangePart = parts[Math.max(0, Math.min(i, parts.length - 1))]!;
        return [
          makeDiagnostic(
            "compile",
            "This program never settles into a finished state.",
            rangeFromTokens(rangePart.tokens, lines),
            lines,
            "Check for a loop that never stops.",
          ),
        ];
      }
      executedParts += 1;
      const part = parts[i];
      if (!part.tokens.length) {
        i = continueCompletedWhileLoops(i + 1);
        continue;
      }
      const statementRange = rangeFromTokens(part.tokens, lines);
      const parsed = parseStatementTokens(part.tokens);
      if (!parsed) {
        return [diagnoseStatementSyntax(part.tokens, lines)];
      }

      if (parsed.kind === "if") {
        const block = ifBlocks.map.get(i);
        if (!block) {
          const nextPart = parts[i + 1];
          if (!nextPart) {
            return [
              makeDiagnostic(
                "compile",
                "This if statement needs a body after it.",
                statementRange,
                lines,
              ),
            ];
          }
          if (!isBracePart(nextPart, "{") && isDeclarationPart(nextPart)) {
            return [
              makeDiagnostic(
                "compile",
                "A declaration directly under if needs braces.",
                statementRange,
                lines,
                "Write if (...) { int x = ...; }",
              ),
            ];
          }
          return [
            makeDiagnostic(
              "compile",
              "This if statement is incomplete.",
              statementRange,
              lines,
            ),
          ];
        }
        const conditionTokens = part.tokens.slice(2, controlHeaderEndIndex(part.tokens));
        const conditionDiagnostic = expressionFailureDiagnostic(
          conditionTokens,
          parsed.expr,
          state,
          declared,
          lines,
          { statementRange },
        );
        if (conditionDiagnostic) return [conditionDiagnostic];
        const condition = evaluateCondition(parsed.expr, state);
        if ("error" in condition) {
          return [
            makeDiagnostic(
              condition.kind,
              condition.kind === "ub"
                ? "This if condition has undefined behavior with the current values."
                : "This if condition does not compile.",
              statementRange,
              lines,
            ),
          ];
        }
        ifDecisions.set(block.headerIndex, condition.value);
        if (condition.value) {
          i += 1;
          continue;
        }
        const elseEntry = elseEntryForIfBlock(block);
        if (elseEntry) {
          if (elseEntry.pushScope) scopes.push(new Set<string>());
          i = continueCompletedWhileLoops(elseEntry.nextIndex);
          continue;
        }
        i = continueCompletedWhileLoops(block.closeIndex + 1);
        continue;
      }

      if (parsed.kind === "while") {
        const block = whileBlocks.map.get(i);
        if (!block) {
          const nextPart = parts[i + 1];
          if (!nextPart) {
            return [
              makeDiagnostic(
                "compile",
                "This while loop needs a body after it.",
                statementRange,
                lines,
              ),
            ];
          }
          if (!isBracePart(nextPart, "{") && isDeclarationPart(nextPart)) {
            return [
              makeDiagnostic(
                "compile",
                "A declaration directly under while needs braces.",
                statementRange,
                lines,
                "Write while (...) { int x = ...; }",
              ),
            ];
          }
          return [
            makeDiagnostic(
              "compile",
              "This while loop is incomplete.",
              statementRange,
              lines,
            ),
          ];
        }
        const conditionTokens = part.tokens.slice(2, controlHeaderEndIndex(part.tokens));
        const conditionDiagnostic = expressionFailureDiagnostic(
          conditionTokens,
          parsed.expr,
          state,
          declared,
          lines,
          { statementRange },
        );
        if (conditionDiagnostic) return [conditionDiagnostic];
        const condition = evaluateCondition(parsed.expr, state);
        if ("error" in condition) {
          return [
            makeDiagnostic(
              condition.kind,
              condition.kind === "ub"
                ? "This while condition has undefined behavior with the current values."
                : "This while condition does not compile.",
              statementRange,
              lines,
            ),
          ];
        }
        if (condition.value) {
          whileStack.push(block);
          i = block.openIndex;
          continue;
        }
        i = continueCompletedWhileLoops(block.afterIndex);
        continue;
      }

      if (parsed.kind === "else") {
        const block = elseLookup.get(i);
        if (!block) {
          return [
            makeDiagnostic(
              "compile",
              "This else does not have a matching if.",
              statementRange,
              lines,
            ),
          ];
        }
        const decision = ifDecisions.get(block.headerIndex);
        if (decision == null) {
          return [
            makeDiagnostic(
              "compile",
              "This else appears before its matching if can be understood.",
              statementRange,
              lines,
            ),
          ];
        }
        if (decision) {
          i = continueCompletedWhileLoops(block.afterIndex);
          continue;
        }
        const elseEntry = elseEntryForIfBlock(block);
        if (!elseEntry) {
          return [
            makeDiagnostic(
              "compile",
              "This else needs a body after it.",
              statementRange,
              lines,
            ),
          ];
        }
        if (elseEntry.pushScope) scopes.push(new Set<string>());
        i = continueCompletedWhileLoops(elseEntry.nextIndex);
        continue;
      }

      if (parsed.kind === "blockStart") {
        scopes.push(new Set<string>());
        i = continueCompletedWhileLoops(i + 1);
        continue;
      }

      if (parsed.kind === "blockEnd") {
        const popped = popScope(scopes, declared, state);
        if (popped.error) {
          return [
            makeDiagnostic(
              "compile",
              "This } does not match any earlier {.",
              statementRange,
              lines,
            ),
          ];
        }
        state = popped.state;
        i = continueCompletedWhileLoops(i + 1);
        continue;
      }

      if (!part.hasSemicolon) {
        return [missingSemicolonDiagnostic(lines, part.endLine + 1)];
      }

      if (isDeclLikeStatement(parsed) && declared.has(parsed.name)) {
        const nameToken = findIdentifierToken(part.tokens, parsed.name);
        return [
          makeDiagnostic(
            "compile",
            `You already declared ${JSON.stringify(parsed.name)} in this scope.`,
            nameToken ? rangeFromToken(nameToken, lines) : statementRange,
            lines,
            "Reuse the existing variable or pick a new name.",
          ),
        ];
      }

      if (parsed.kind === "declAssign") {
        const assignIndex = findTopLevelAssignmentOperatorIndex(part.tokens);
        const rhsTokens =
          assignIndex >= 0 ? part.tokens.slice(assignIndex + 1) : part.tokens;
        const rhsRange = rangeFromTokens(
          rhsTokens.length ? rhsTokens : part.tokens,
          lines,
        );
        const diagnostic = expressionFailureDiagnostic(
          rhsTokens.length ? rhsTokens : part.tokens,
          parsed.expr,
          state,
          declared,
          lines,
          {
            targetType: parsed.declType || "int",
            statementRange: rhsRange,
          },
        );
        if (diagnostic) return [diagnostic];
      } else if (parsed.kind === "assign") {
        const assignIndex = findTopLevelAssignmentOperatorIndex(part.tokens);
        const lhsTokens =
          assignIndex > 0 ? part.tokens.slice(0, assignIndex) : part.tokens;
        const rhsTokens =
          assignIndex >= 0 ? part.tokens.slice(assignIndex + 1) : part.tokens;
        const target = diagnoseWritableTarget(
          lhsTokens.length ? lhsTokens : part.tokens,
          parsed.lhs,
          state,
          declared,
          lines,
          parsed.op !== "=",
        );
        if ("diagnostic" in target) return [target.diagnostic];
        if (parsed.op === "=") {
          const rhsRange = rangeFromTokens(
            rhsTokens.length ? rhsTokens : part.tokens,
            lines,
          );
          const diagnostic = expressionFailureDiagnostic(
            rhsTokens.length ? rhsTokens : part.tokens,
            parsed.rhs,
            state,
            declared,
            lines,
            {
              targetType: target.targetType,
              statementRange: rhsRange,
            },
          );
          if (diagnostic) return [diagnostic];
        } else {
          const diagnostic = expressionFailureDiagnostic(
            part.tokens,
            {
              kind: "assign",
              op: parsed.op,
              left: parsed.lhs,
              right: parsed.rhs,
            },
            state,
            declared,
            lines,
            {
              targetType: target.targetType,
              statementRange,
            },
          );
          if (diagnostic) return [diagnostic];
        }
      } else if (parsed.kind === "expr") {
        const diagnostic = expressionFailureDiagnostic(
          part.tokens,
          parsed.expr,
          state,
          declared,
          lines,
          { statementRange },
        );
        if (diagnostic) return [diagnostic];
      }

      const next = applyStatement(state, parsed, {
        alloc,
        allowRedeclare: false,
      });
      if (!next) {
        return [
          makeDiagnostic(
            "compile",
            "This statement does not work as written.",
            statementRange,
            lines,
          ),
        ];
      }
      if (isDeclLikeStatement(parsed)) {
        addDeclaredNames(scopes, declared, parsed.declaredNames);
      }
      state = next;
      i = continueCompletedWhileLoops(i + 1);
    }

    return [];
  }

  return {
    tokenizeProgram,
    splitStatements,
    parseStatements,
    buildStatementMap,
    buildIfStatementMap,
    buildWhileStatementMap,
    statementRangeForLine,
    getStatementContext,
    evaluateCondition,
    evaluateExpressionText,
    findMissingSemicolonLines,
    applyProgramParts,
    analyzeProgramParts,
    traceProgramParts,
    diagnoseProgram,
    applyProgram,
  };
}
