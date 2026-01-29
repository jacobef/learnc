import type { BoxState, BoxValue } from "./shared-core-utils.js";
import {
  cloneBoxes,
  formatValueForType,
  normalizeSpecialFloatLiteral,
  parseDoubleValueWithSign,
  parseType,
  randAddr,
  stripAllComments,
} from "./shared-core-utils.js";

export type ExprNode =
  | { kind: "num"; value: string }
  | { kind: "var"; name: string }
  | { kind: "unary"; op: "+" | "-" | "*" | "&" | "~"; expr: ExprNode }
  | {
      kind: "binary";
      op:
        | "+"
        | "-"
        | "*"
        | "/"
        | "=="
        | "!="
        | ">"
        | ">="
        | "<"
        | "<="
        | "<<"
        | ">>"
        | "&"
        | "^"
        | "|";
      left: ExprNode;
      right: ExprNode;
    };
export type AssignRhs =
  | { kind: "num"; value: string }
  | { kind: "var"; name: string }
  | { kind: "ref"; name: string }
  | { kind: "deref"; name: string; depth: number }
  | { kind: "unary"; name: string; ops: string[] }
  | { kind: "expr"; expr: ExprNode; hasVar: boolean };
export type Statement =
  | { kind: "blockStart" }
  | { kind: "blockEnd" }
  | { kind: "if"; expr: ExprNode; hasVar: boolean }
  | { kind: "decl"; name: string; type: string }
  | { kind: "assign"; name: string; value: string; valueKind: "num" }
  | {
      kind: "assign";
      name: string;
      valueKind: "expr";
      expr: ExprNode;
      hasVar: boolean;
    }
  | { kind: "assignVar"; name: string; src: string }
  | { kind: "assignRef"; name: string; ref: string }
  | { kind: "assignDeref"; name: string; value: string; depth?: number }
  | { kind: "assignDerefVar"; name: string; src: string; depth?: number }
  | { kind: "assignDerefRef"; name: string; ref: string; depth?: number }
  | { kind: "assignFromDeref"; name: string; ptr: string; depth?: number }
  | { kind: "assignUnary"; name: string; ops: string[]; rhs: AssignRhs }
  | { kind: "assignUnaryRhs"; name: string; src: string; ops: string[] }
  | {
      kind: "declAssign";
      name: string;
      declType?: string;
      value: string;
      valueKind: "num";
    }
  | {
      kind: "declAssign";
      name: string;
      declType?: string;
      valueKind: "expr";
      expr: ExprNode;
      hasVar: boolean;
    }
  | { kind: "declAssignVar"; name: string; src: string; declType?: string }
  | { kind: "declAssignRef"; name: string; ref: string; declType?: string }
  | {
      kind: "declAssignDeref";
      name: string;
      ptr: string;
      depth?: number;
      declType?: string;
    }
  | {
      kind: "declAssignUnary";
      name: string;
      src: string;
      ops: string[];
      declType?: string;
    };
export interface Token {
  type: "kw" | "ident" | "number" | "sym" | "unknown";
  value: string;
  line: number;
  col: number;
}
export interface StatementPart {
  tokens: Token[];
  startLine: number;
  endLine: number;
  hasSemicolon: boolean;
}
export type StatementRange = {
  startLine: number;
  endLine: number;
  hasSemicolon: boolean;
};
export type StatementMap = {
  parts: StatementPart[];
  byLine: Array<StatementRange | null>;
};
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
  expr: ExprNode;
  hasVar: boolean;
};
export type IfBlockMap = {
  map: Map<number, IfBlock>;
  errors: Map<number, string>;
  incomplete: Set<number>;
};
export type ConditionResult =
  | { value: boolean }
  | { error: string | { text: string; html: string }; kind: "compile" | "ub" };
export type ProgramResult =
  | { kind: "ok"; state: BoxState[] }
  | { kind: "compile" | "ub" };
export interface LineStatus {
  invalid: Set<number>;
  incomplete: Set<number>;
  errors: Map<number, string | { text: string; html: string }>;
  errorKinds: Map<number, string>;
  info: Map<number, string | { text: string; html: string }>;
}
export interface SimpleSimulator {
  tokenizeProgram: (src?: string) => Token[];
  splitStatements: (tokens: Token[]) => StatementPart[];
  parseStatements: (text: string) => Statement[];
  buildStatementMap: (lines: string[]) => StatementMap;
  buildIfStatementMap: (
    parts: StatementPart[],
    opts?: { lastLine?: number },
  ) => IfBlockMap;
  statementRangeForLine: (
    statementMap: StatementMap | null,
    lineIndex: number,
  ) => StatementRange | null;
  getStatementContext: (lines: string[], boundary: number) => StatementContext;
  evaluateCondition: (expr: ExprNode, state: BoxState[]) => ConditionResult;
  evaluateExpressionText: (
    expr: string,
    state: BoxState[],
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
        error: string | { text: string; html: string };
        kind: "compile" | "ub";
      };
  classifyLineStatuses: (
    lines: string[],
    opts?: { alloc?: (type?: string) => string },
  ) => LineStatus;
  findMissingSemicolonLines: (text: string) => number[];
  applyProgramParts: (
    parts: StatementPart[],
    opts?: { alloc?: (type?: string) => string; stop?: number },
  ) => BoxState[] | null;
  analyzeProgramParts: (
    parts: StatementPart[],
    opts?: { alloc?: (type?: string) => string; stop?: number },
  ) => ProgramResult;
  applyProgram: (
    text: string,
    opts?: { alloc?: (type?: string) => string },
  ) => BoxState[] | null;
}

export function createSimpleSimulator(
  opts: {
    allowVarAssign?: boolean;
    requireSourceValue?: boolean;
    allowPointers?: boolean;
  } = {},
): SimpleSimulator {
  const {
    allowVarAssign = false,
    requireSourceValue = false,
    allowPointers = false,
  } = opts;

  type DeclaredNames = Set<string>;
  type ScopeStack = Array<Set<string>>;
  type ExprParseResult = {
    expr: ExprNode;
    nextIndex: number;
    hasVar: boolean;
  };
  type UnaryLhs = { ops: string[]; name: string; idx: number };
  type EvalError = {
    error: string | { text: string; html: string };
    kind: "compile" | "ub";
    base?: string;
    depth?: number;
    value?: BoxValue | bigint | number;
    address?: string;
    label?: string;
  };
  type EvalValue = {
    kind: "lvalue" | "rvalue";
    base: string;
    depth: number;
    value: BoxValue | bigint | number;
    address: string;
    label: string;
    nanSign?: -1 | 1;
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
  type UnaryResult =
    | {
        kind: "lvalue";
        type: string;
        value: BoxValue;
        address: string;
        box: BoxState;
        refBox?: BoxState | null;
        nanSign?: -1 | 1;
      }
    | {
        kind: "rvalue";
        type: string;
        value: BoxValue;
        address: string;
        box: null;
        refBox?: BoxState | null;
        nanSign?: -1 | 1;
      };
  type StatementValidationResult =
    | {
        error: string | { text: string; html: string };
        kind: "compile" | "ub";
      }
    | { parsed: Statement; next: BoxState[] };
  const isEvalError = (result: EvalResult): result is EvalError =>
    !!result.error;
  const isScalarError = (result: ScalarResult): result is EvalError =>
    !!result.error;

  function tokenizeProgram(src = ""): Token[] {
    const tokens: Token[] = [];
    let i = 0;
    let line = 0;
    let col = 0;
    while (i < src.length) {
      const ch = src[i];
      if (ch === "\r") {
        i++;
        if (src[i] === "\n") i++;
        line++;
        col = 0;
        continue;
      }
      if (ch === "\n") {
        i++;
        line++;
        col = 0;
        continue;
      }
      if (ch === "/" && src[i + 1] === "/") {
        i += 2;
        col += 2;
        while (i < src.length && src[i] !== "\n" && src[i] !== "\r") {
          i++;
          col++;
        }
        continue;
      }
      if (ch === "/" && src[i + 1] === "*") {
        const startLine = line;
        const startCol = col;
        i += 2;
        col += 2;
        let closed = false;
        while (i < src.length) {
          const c = src[i];
          if (c === "\r") {
            i++;
            if (src[i] === "\n") i++;
            line++;
            col = 0;
            continue;
          }
          if (c === "\n") {
            i++;
            line++;
            col = 0;
            continue;
          }
          if (c === "*" && src[i + 1] === "/") {
            i += 2;
            col += 2;
            closed = true;
            break;
          }
          i++;
          col++;
        }
        if (!closed) {
          tokens.push({
            type: "unknown",
            value: "/*",
            line: startLine,
            col: startCol,
          });
        }
        continue;
      }
      if (/\s/.test(ch)) {
        i++;
        col++;
        continue;
      }
      if (ch === "=") {
        if (src[i + 1] === "=") {
          tokens.push({ type: "sym", value: "==", line, col });
          i += 2;
          col += 2;
        } else {
          tokens.push({ type: "sym", value: ch, line, col });
          i++;
          col++;
        }
        continue;
      }
      if (ch === "!") {
        if (src[i + 1] === "=") {
          tokens.push({ type: "sym", value: "!=", line, col });
          i += 2;
          col += 2;
        } else {
          tokens.push({ type: "unknown", value: ch, line, col });
          i++;
          col++;
        }
        continue;
      }
      if (ch === "<" || ch === ">") {
        if (src[i + 1] === ch) {
          tokens.push({ type: "sym", value: `${ch}${ch}`, line, col });
          i += 2;
          col += 2;
        } else if (src[i + 1] === "=") {
          tokens.push({ type: "sym", value: `${ch}=`, line, col });
          i += 2;
          col += 2;
        } else {
          tokens.push({ type: "sym", value: ch, line, col });
          i++;
          col++;
        }
        continue;
      }
      if (
        ch === "+" ||
        ch === "-" ||
        ch === "*" ||
        ch === "/" ||
        ch === "&" ||
        ch === "|" ||
        ch === "^" ||
        ch === "~" ||
        ch === ";" ||
        ch === "(" ||
        ch === ")" ||
        ch === "{" ||
        ch === "}"
      ) {
        tokens.push({ type: "sym", value: ch, line, col });
        i++;
        col++;
        continue;
      }
      if (/[A-Za-z_]/.test(ch)) {
        const startCol = col;
        let j = i + 1;
        while (j < src.length && /[A-Za-z0-9_]/.test(src[j])) j++;
        const ident = src.slice(i, j);
        const special = normalizeSpecialFloatLiteral(ident);
        tokens.push({
          type: special
            ? "number"
            : ident === "int" ||
                ident === "long" ||
                ident === "double" ||
                ident === "if" ||
                ident === "else"
              ? "kw"
              : "ident",
          value: special || ident,
          line,
          col: startCol,
        });
        col += j - i;
        i = j;
        continue;
      }
      if (/[0-9]/.test(ch)) {
        const startCol = col;
        let j = i;
        if (src[j] === "0" && (src[j + 1] === "x" || src[j + 1] === "X")) {
          j += 2;
          while (j < src.length && /[0-9a-fA-F]/.test(src[j])) j++;
        } else {
          while (j < src.length && /[0-9]/.test(src[j])) j++;
          if (src[j] === ".") {
            j++;
            while (j < src.length && /[0-9]/.test(src[j])) j++;
          }
          if (src[j] === "e" || src[j] === "E") {
            let k = j + 1;
            if (src[k] === "+" || src[k] === "-") k++;
            const expStart = k;
            while (k < src.length && /[0-9]/.test(src[k])) k++;
            if (k > expStart) {
              j = k;
            }
          }
        }
        tokens.push({
          type: "number",
          value: src.slice(i, j),
          line,
          col: startCol,
        });
        col += j - i;
        i = j;
        continue;
      }
      if (ch === "." && /[0-9]/.test(src[i + 1] || "")) {
        const startCol = col;
        let j = i + 1;
        while (j < src.length && /[0-9]/.test(src[j])) j++;
        if (src[j] === "e" || src[j] === "E") {
          let k = j + 1;
          if (src[k] === "+" || src[k] === "-") k++;
          const expStart = k;
          while (k < src.length && /[0-9]/.test(src[k])) k++;
          if (k > expStart) {
            j = k;
          }
        }
        tokens.push({
          type: "number",
          value: `0${src.slice(i, j)}`,
          line,
          col: startCol,
        });
        col += j - i;
        i = j;
        continue;
      }
      tokens.push({ type: "unknown", value: ch, line, col });
      i++;
      col++;
    }
    return tokens;
  }

  function hasDeclaredPrefix(prefix: string, names: DeclaredNames | null) {
    if (!prefix || !names || !names.size) return false;
    for (const name of names) {
      if (name.startsWith(prefix)) return true;
    }
    return false;
  }

  function isBraceToken(tok: Token): boolean {
    return tok.type === "sym" && (tok.value === "{" || tok.value === "}");
  }

  function addDeclaredName(
    scopes: ScopeStack,
    declared: DeclaredNames,
    name: string,
  ) {
    declared.add(name);
    const current = scopes[scopes.length - 1];
    if (current) current.add(name);
  }

  function popScope(
    scopes: ScopeStack,
    declared: DeclaredNames,
    state: BoxState[],
  ): { state: BoxState[]; error?: string } {
    if (scopes.length <= 1) {
      return { state, error: "Unexpected }." };
    }
    const frame = scopes.pop();
    if (!frame || frame.size === 0) return { state };
    const namesToRemove = new Set(frame);
    const nextState = state.filter((box) => !namesToRemove.has(box.name));
    frame.forEach((name) => declared.delete(name));
    return { state: nextState };
  }

  function resolveDeclType(
    stars: number,
    baseType: string = "int",
  ): string | null {
    if (!Number.isFinite(stars) || stars < 0) return null;
    if (
      !baseType ||
      (baseType !== "int" && baseType !== "long" && baseType !== "double")
    )
      return null;
    if (stars === 0) return baseType;
    if (!allowPointers) return null;
    return `${baseType}${"*".repeat(stars)}`;
  }

  function isPointerType(type: string): boolean {
    const { depth } = parseType(type);
    return Number.isFinite(depth) && depth > 0;
  }

  function pointerDepth(type: string): number | null {
    const { depth } = parseType(type);
    return depth;
  }

  function makePointerType(depth: number, base: string = "int"): string | null {
    if (!Number.isFinite(depth) || depth < 0) return null;
    if (base !== "int" && base !== "long" && base !== "double") return null;
    return depth === 0 ? base : `${base}${"*".repeat(depth)}`;
  }

  const INT32_MIN = -2147483648n;
  const INT32_MAX = 2147483647n;
  const INT64_MIN = -9223372036854775808n;
  const INT64_MAX = 9223372036854775807n;

  function parseIntegerLiteral(value: string): bigint | null {
    const raw = String(value ?? "").trim();
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

  function classifyNumericLiteral(value: string): "ok" | "compile" | "ub" {
    const n = parseIntegerLiteral(value);
    if (n == null) return "compile";
    if (n < INT64_MIN || n > INT64_MAX) return "compile";
    if (n < INT32_MIN || n > INT32_MAX) return "ub";
    return "ok";
  }

  function isSpecialFloatLiteral(value: string): boolean {
    return !!normalizeSpecialFloatLiteral(value);
  }

  function isDecimalLiteral(value: string): boolean {
    return /[.eE]/.test(String(value));
  }

  function numericLiteralErrorForType(
    value: string,
    type: string,
  ): EvalError | null {
    const { base, depth } = parseType(type);
    if (!base || depth !== 0) return null;
    if (isSpecialFloatLiteral(value)) {
      if (base === "double") return null;
      return { error: "That number isn't an integer.", kind: "compile" };
    }
    if (isDecimalLiteral(value)) {
      const parsed = Number(value);
      if (Number.isNaN(parsed)) {
        return {
          error: "That number is too large to represent.",
          kind: "compile",
        };
      }
      if (!Number.isFinite(parsed)) {
        return {
          error: "That number is too large to represent.",
          kind: "compile",
        };
      }
      return null;
    }
    if (base === "double") return null;
    const status = classifyNumericLiteral(value);
    if (base === "long") {
      return status === "compile" ? numericLiteralError(status) : null;
    }
    if (base === "int") return numericLiteralError(status);
    return null;
  }

  function numericLiteralError(
    kind: "ok" | "compile" | "ub",
  ): EvalError | null {
    if (kind === "compile")
      return {
        error: "That number is too large to represent.",
        kind: "compile",
      };
    if (kind === "ub")
      return { error: "That number is too large for int.", kind: "ub" };
    return null;
  }

  function integerRangeForBase(
    base: string,
  ): { min: bigint; max: bigint } | null {
    if (base === "int") return { min: INT32_MIN, max: INT32_MAX };
    if (base === "long") return { min: INT64_MIN, max: INT64_MAX };
    return null;
  }

  function integerOverflowError(base: string): EvalError {
    const article = base === "int" ? "an" : "a";
    return {
      error: `That calculation overflows ${article} ${base}.`,
      kind: "ub",
    };
  }

  function checkIntegerRange(value: bigint, base: string): EvalError | null {
    const range = integerRangeForBase(base);
    if (!range) return null;
    if (value < range.min || value > range.max)
      return integerOverflowError(base);
    return null;
  }

  function bitWidthForBase(base: string): number | null {
    if (base === "int") return 32;
    if (base === "long") return 64;
    return null;
  }

  function isRefCompatible(targetType: string, refType: string): boolean {
    const { base: targetBase, depth: targetDepth } = parseType(targetType);
    const { base: refBase, depth: refDepth } = parseType(refType);
    if (
      !targetBase ||
      !refBase ||
      !Number.isFinite(targetDepth) ||
      !Number.isFinite(refDepth)
    )
      return false;
    return targetBase === refBase && targetDepth === refDepth + 1;
  }

  function expectedPointerTypeForRef(refType: string): string | null {
    const { base, depth } = parseType(refType);
    if (!base || !Number.isFinite(depth)) return null;
    return makePointerType(depth + 1, base);
  }

  function isExpressionPrefix(
    tokens: Token[],
    { allowVars = true }: { allowVars?: boolean } = {},
  ): boolean {
    if (!tokens.length) return true;
    let expectingOperand = true;
    let depth = 0;
    for (const tok of tokens) {
      if (tok.type === "unknown") return false;
      if (expectingOperand) {
        if (tok.type === "number") {
          expectingOperand = false;
          continue;
        }
        if (tok.type === "ident") {
          if (!allowVars) return false;
          expectingOperand = false;
          continue;
        }
        if (tok.type === "sym") {
          if (tok.value === "(") {
            depth++;
            continue;
          }
          if (tok.value === "+" || tok.value === "-" || tok.value === "~") {
            continue;
          }
          if (allowPointers && (tok.value === "*" || tok.value === "&")) {
            continue;
          }
        }
        return false;
      } else {
        if (tok.type === "sym") {
          if (tok.value === ")") {
            if (depth <= 0) return false;
            depth--;
            continue;
          }
          if (
            tok.value === "+" ||
            tok.value === "-" ||
            tok.value === "*" ||
            tok.value === "/" ||
            tok.value === "==" ||
            tok.value === "!=" ||
            tok.value === "<<" ||
            tok.value === ">>" ||
            tok.value === "<" ||
            tok.value === "<=" ||
            tok.value === ">" ||
            tok.value === ">=" ||
            tok.value === "&" ||
            tok.value === "^" ||
            tok.value === "|"
          ) {
            expectingOperand = true;
            continue;
          }
        }
        return false;
      }
    }
    return true;
  }

  function isDeclPrefix(tokens: Token[]): boolean {
    if (!tokens.length) return false;
    if (tokens[0].type !== "kw") return false;
    const baseType = tokens[0].value;
    if (baseType !== "int" && baseType !== "long" && baseType !== "double")
      return false;
    let idx = 1;
    let stars = 0;
    while (
      idx < tokens.length &&
      tokens[idx].type === "sym" &&
      tokens[idx].value === "*"
    ) {
      if (!allowPointers) return false;
      stars++;
      idx++;
    }
    if (!resolveDeclType(stars, baseType)) return false;
    if (idx === tokens.length) return true;
    if (tokens[idx].type !== "ident") return false;
    idx++;
    if (idx === tokens.length) return true;
    if (tokens[idx].type !== "sym" || tokens[idx].value !== "=") return false;
    idx++;
    if (idx === tokens.length) return true;
    return isExpressionPrefix(tokens.slice(idx), { allowVars: allowVarAssign });
  }

  function isAssignPrefix(
    tokens: Token[],
    declaredNames: DeclaredNames,
  ): boolean {
    if (!tokens.length) return false;
    if (tokens[0].type !== "ident") return false;
    const name = tokens[0].value;
    if (tokens.length === 1) return hasDeclaredPrefix(name, declaredNames);
    if (tokens[1].type !== "sym" || tokens[1].value !== "=") return false;
    if (!declaredNames?.has(name)) return false;
    if (tokens.length === 2) return true;
    const t2 = tokens[2];
    if (
      t2.type === "sym" &&
      (t2.value === "&" || t2.value === "*") &&
      allowPointers
    ) {
      let j = 2;
      while (
        j < tokens.length &&
        tokens[j].type === "sym" &&
        (tokens[j].value === "*" || tokens[j].value === "&")
      )
        j++;
      if (j === tokens.length) return true;
      if (tokens[j].type !== "ident") return false;
      return (
        j === tokens.length - 1 &&
        hasDeclaredPrefix(tokens[j].value, declaredNames)
      );
    }
    return isExpressionPrefix(tokens.slice(2), { allowVars: allowVarAssign });
  }

  function isUnaryAssignPrefix(
    tokens: Token[],
    declaredNames: DeclaredNames,
  ): boolean {
    if (!allowPointers) return false;
    if (!tokens.length) return false;
    let idx = 0;
    while (
      idx < tokens.length &&
      tokens[idx].type === "sym" &&
      (tokens[idx].value === "*" || tokens[idx].value === "&")
    )
      idx++;
    if (idx === 0) return false;
    if (idx === tokens.length) return true;
    if (tokens[idx].type !== "ident") return false;
    const name = tokens[idx].value;
    if (idx === tokens.length - 1)
      return hasDeclaredPrefix(name, declaredNames);
    idx++;
    if (tokens[idx].type !== "sym" || tokens[idx].value !== "=") return false;
    idx++;
    if (idx >= tokens.length) return true;
    const rhs = tokens[idx];
    if (
      rhs.type === "sym" &&
      (rhs.value === "&" || rhs.value === "*") &&
      allowPointers
    ) {
      let j = idx;
      while (
        j < tokens.length &&
        tokens[j].type === "sym" &&
        (tokens[j].value === "*" || tokens[j].value === "&")
      )
        j++;
      if (j === tokens.length) return true;
      if (tokens[j].type !== "ident") return false;
      return (
        j === tokens.length - 1 &&
        hasDeclaredPrefix(tokens[j].value, declaredNames)
      );
    }
    return isExpressionPrefix(tokens.slice(idx), {
      allowVars: allowVarAssign,
    });
  }

  function isDerefPrefix(
    tokens: Token[],
    declaredNames: DeclaredNames,
  ): boolean {
    if (!allowPointers) return false;
    if (!tokens.length) return false;
    let idx = 0;
    while (
      idx < tokens.length &&
      tokens[idx].type === "sym" &&
      tokens[idx].value === "*"
    )
      idx++;
    if (idx === 0) return false;
    if (idx === tokens.length) return true;
    if (tokens[idx].type !== "ident") return false;
    const name = tokens[idx].value;
    if (idx === tokens.length - 1)
      return hasDeclaredPrefix(name, declaredNames);
    idx++;
    if (tokens[idx].type !== "sym" || tokens[idx].value !== "=") return false;
    if (!declaredNames?.has(name)) return false;
    idx++;
    if (idx >= tokens.length) return true;
    const rhs = tokens[idx];
    if (rhs.type === "sym" && rhs.value === "&" && allowPointers) {
      if (idx === tokens.length - 1) return true;
      return idx === tokens.length - 2 && tokens[idx + 1].type === "ident";
    }
    return isExpressionPrefix(tokens.slice(idx), {
      allowVars: allowVarAssign,
    });
  }

  function isIfPrefix(tokens: Token[]): boolean {
    if (!tokens.length) return false;
    if (tokens[0].type !== "kw" || tokens[0].value !== "if") return false;
    if (tokens.length === 1) return true;
    if (tokens[1].type !== "sym" || tokens[1].value !== "(") return false;
    let depth = 0;
    for (let i = 1; i < tokens.length; i++) {
      const tok = tokens[i];
      if (tok.type === "sym" && tok.value === "(") {
        depth++;
        continue;
      }
      if (tok.type === "sym" && tok.value === ")") {
        depth--;
        if (depth === 0) {
          return i === tokens.length - 1;
        }
      }
    }
    return true;
  }

  function isStatementPrefix(
    tokens: Token[],
    declaredNames: DeclaredNames,
    allowIntPrefix: boolean,
  ): boolean {
    if (!tokens.length) return false;
    if (tokens.some((t) => t.type === "unknown")) return false;
    if (tokens.length === 1) {
      const t0 = tokens[0];
      if (
        t0.type === "kw" &&
        (t0.value === "int" || t0.value === "long" || t0.value === "double")
      )
        return true;
      if (t0.type === "sym" && (t0.value === "{" || t0.value === "}"))
        return true;
      if (t0.type === "ident") {
        if (
          allowIntPrefix &&
          ("int".startsWith(t0.value) ||
            "long".startsWith(t0.value) ||
            "double".startsWith(t0.value))
        )
          return true;
        return hasDeclaredPrefix(t0.value, declaredNames);
      }
      if (allowPointers && t0.type === "sym" && t0.value === "*") return true;
    }
    return (
      isIfPrefix(tokens) ||
      isDeclPrefix(tokens) ||
      isAssignPrefix(tokens, declaredNames) ||
      isDerefPrefix(tokens, declaredNames) ||
      isUnaryAssignPrefix(tokens, declaredNames)
    );
  }

  function exprHasVar(node: ExprNode | null): boolean {
    if (!node) return false;
    if (node.kind === "var") return true;
    if (node.kind === "unary") return exprHasVar(node.expr);
    if (node.kind === "binary")
      return exprHasVar(node.left) || exprHasVar(node.right);
    return false;
  }

  function parseExpressionTokens(
    tokens: Token[],
    start: number,
    { allowVars = true }: { allowVars?: boolean } = {},
  ): ExprParseResult | null {
    let idx = start;
    const next = () => tokens[idx];

    function parsePrimary(): ExprNode | null {
      const tok = next();
      if (!tok) return null;
      if (tok.type === "number") {
        idx++;
        return { kind: "num", value: tok.value };
      }
      if (tok.type === "ident") {
        if (!allowVars) return null;
        idx++;
        return { kind: "var", name: tok.value };
      }
      if (tok.type === "sym" && tok.value === "(") {
        idx++;
        const expr = parseBitwiseOr();
        if (!expr) return null;
        const close = next();
        if (!close || close.type !== "sym" || close.value !== ")") return null;
        idx++;
        return expr;
      }
      return null;
    }

    function parseUnary(): ExprNode | null {
      const tok = next();
      if (
        tok &&
        tok.type === "sym" &&
        (tok.value === "+" ||
          tok.value === "-" ||
          tok.value === "~" ||
          (allowPointers && (tok.value === "*" || tok.value === "&")))
      ) {
        idx++;
        const expr = parseUnary();
        if (!expr) return null;
        return { kind: "unary", op: tok.value, expr };
      }
      return parsePrimary();
    }

    function parseMulDiv(): ExprNode | null {
      let left = parseUnary();
      if (!left) return null;
      while (true) {
        const tok = next();
        if (
          !tok ||
          tok.type !== "sym" ||
          (tok.value !== "*" && tok.value !== "/")
        )
          break;
        idx++;
        const right = parseUnary();
        if (!right) return null;
        left = { kind: "binary", op: tok.value, left, right };
      }
      return left;
    }

    function parseAddSub(): ExprNode | null {
      let left = parseMulDiv();
      if (!left) return null;
      while (true) {
        const tok = next();
        if (
          !tok ||
          tok.type !== "sym" ||
          (tok.value !== "+" && tok.value !== "-")
        )
          break;
        idx++;
        const right = parseMulDiv();
        if (!right) return null;
        left = { kind: "binary", op: tok.value, left, right };
      }
      return left;
    }

    function parseShift(): ExprNode | null {
      let left = parseAddSub();
      if (!left) return null;
      while (true) {
        const tok = next();
        if (
          !tok ||
          tok.type !== "sym" ||
          (tok.value !== "<<" && tok.value !== ">>")
        )
          break;
        idx++;
        const right = parseAddSub();
        if (!right) return null;
        left = { kind: "binary", op: tok.value, left, right };
      }
      return left;
    }

    function parseRelational(): ExprNode | null {
      let left = parseShift();
      if (!left) return null;
      while (true) {
        const tok = next();
        if (
          !tok ||
          tok.type !== "sym" ||
          (tok.value !== "<" &&
            tok.value !== "<=" &&
            tok.value !== ">" &&
            tok.value !== ">=")
        )
          break;
        idx++;
        const right = parseShift();
        if (!right) return null;
        left = { kind: "binary", op: tok.value, left, right };
      }
      return left;
    }

    function parseEquality(): ExprNode | null {
      let left = parseRelational();
      if (!left) return null;
      while (true) {
        const tok = next();
        if (
          !tok ||
          tok.type !== "sym" ||
          (tok.value !== "==" && tok.value !== "!=")
        )
          break;
        idx++;
        const right = parseRelational();
        if (!right) return null;
        left = { kind: "binary", op: tok.value, left, right };
      }
      return left;
    }

    function parseBitwiseAnd(): ExprNode | null {
      let left = parseEquality();
      if (!left) return null;
      while (true) {
        const tok = next();
        if (!tok || tok.type !== "sym" || tok.value !== "&") break;
        idx++;
        const right = parseEquality();
        if (!right) return null;
        left = { kind: "binary", op: tok.value, left, right };
      }
      return left;
    }

    function parseBitwiseXor(): ExprNode | null {
      let left = parseBitwiseAnd();
      if (!left) return null;
      while (true) {
        const tok = next();
        if (!tok || tok.type !== "sym" || tok.value !== "^") break;
        idx++;
        const right = parseBitwiseAnd();
        if (!right) return null;
        left = { kind: "binary", op: tok.value, left, right };
      }
      return left;
    }

    function parseBitwiseOr(): ExprNode | null {
      let left = parseBitwiseXor();
      if (!left) return null;
      while (true) {
        const tok = next();
        if (!tok || tok.type !== "sym" || tok.value !== "|") break;
        idx++;
        const right = parseBitwiseXor();
        if (!right) return null;
        left = { kind: "binary", op: tok.value, left, right };
      }
      return left;
    }

    const expr = parseBitwiseOr();
    if (!expr) return null;
    return { expr, nextIndex: idx, hasVar: exprHasVar(expr) };
  }

  function coerceScalarResult(
    result: EvalResult | null,
    requireValue: boolean,
  ): ScalarResult {
    if (!result)
      return {
        error: "That expression is not valid here.",
        kind: "compile",
      };
    if (result.error) return result;
    if (!Number.isFinite(result.depth) || result.depth !== 0)
      return {
        error: "Pointer arithmetic is not supported here.",
        kind: "compile",
      };
    const raw = result.value;
    if (result.kind === "lvalue") {
      if (requireValue && String(raw ?? "") === "") {
        const label = result.label || "That value";
        return { error: `${label} doesn't have a value yet.`, kind: "ub" };
      }
    }
    const base = result.base || "int";
    if (base === "double") {
      const rawValue = raw ?? "";
      const parsed = parseDoubleValueWithSign(rawValue);
      if (!parsed) {
        const label = result.label || "That value";
        return { error: `${label} isn't a number.`, kind: "compile" };
      }
      const nanSign =
        "nanSign" in result && result.nanSign !== undefined
          ? result.nanSign
          : parsed.nanSign;
      return { value: parsed.value, base, nanSign };
    }
    try {
      const value = typeof raw === "bigint" ? raw : BigInt(String(raw));
      return { value, base };
    } catch {
      const label = result.label || "That value";
      return { error: `${label} isn't a number.`, kind: "compile" };
    }
  }

  function evaluateExpressionRaw(
    expr: ExprNode,
    state: BoxState[],
    opts: {
      allowVars?: boolean;
      targetType?: string;
      requireValue?: boolean;
    } = {},
  ): EvalResult | EvalError {
    const {
      allowVars = true,
      targetType = "int",
      requireValue = requireSourceValue,
    } = opts;
    const by = Object.fromEntries(state.map((b) => [b.name, b]));
    const targetBase = parseType(targetType).base || "int";
    const toNumber = (value: bigint | number): number =>
      typeof value === "bigint" ? Number(value) : value;

    function makeLvalue(box: BoxState, label: string): EvalResult {
      const { base, depth } = parseType(box.type);
      if (!base || !Number.isFinite(depth)) {
        return {
          error: "That expression is not valid here.",
          kind: "compile",
        };
      }
      let nanSign: -1 | 1 | undefined;
      if (base === "double") {
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
      };
    }

    function makeRvalue(
      value: BoxValue | bigint | number,
      base: string,
      depth: number = 0,
      label: string = "",
      nanSign?: -1 | 1,
    ): EvalResult {
      return {
        kind: "rvalue",
        base,
        depth,
        value,
        address: "",
        label,
        nanSign,
      };
    }

    function evalNode(node: ExprNode | null): EvalResult {
      if (!node)
        return {
          error: "That expression is not valid here.",
          kind: "compile",
        };
      if (node.kind === "num") {
        const err = numericLiteralErrorForType(node.value, targetType);
        if (err) return err;
        try {
          if (
            isDecimalLiteral(node.value) ||
            isSpecialFloatLiteral(node.value)
          ) {
            const parsed = parseDoubleValueWithSign(node.value);
            const value = parsed ? parsed.value : Number(node.value);
            return makeRvalue(value, "double", 0, "", parsed?.nanSign);
          }
          const literalStatus = classifyNumericLiteral(node.value);
          if (literalStatus === "compile") {
            return {
              error: "That number is too large to represent.",
              kind: "compile",
            };
          }
          const literalBase = literalStatus === "ub" ? "long" : "int";
          const intValue = parseIntegerLiteral(node.value);
          if (intValue == null) {
            return {
              error: "That number is too large to represent.",
              kind: "compile",
            };
          }
          return makeRvalue(intValue, literalBase);
        } catch {
          return {
            error: "That number is too large to represent.",
            kind: "compile",
          };
        }
      }
      if (node.kind === "var") {
        if (!allowVars)
          return {
            error: "Assignments should use a number.",
            kind: "compile",
          };
        const box = by[node.name];
        if (!box)
          return {
            error: `You can't use ${node.name} before declaring it.`,
            kind: "compile",
          };
        return makeLvalue(box, node.name);
      }
      if (node.kind === "unary") {
        const rhs = evalNode(node.expr);
        if (isEvalError(rhs)) return rhs;
        const rhsDepth = rhs.depth ?? 0;
        if (node.op === "&") {
          const label = `&${rhs.label || ""}`;
          if (rhs.kind !== "lvalue" || !rhs.address) {
            return { error: `${label} is not valid here.`, kind: "compile" };
          }
          const nextDepth = Number.isFinite(rhsDepth) ? rhsDepth + 1 : 1;
          const nextBase = rhs.base || "int";
          return makeRvalue(String(rhs.address), nextBase, nextDepth, label);
        }
        if (node.op === "*") {
          const label = `*${rhs.label || ""}`;
          if (!Number.isFinite(rhsDepth) || rhsDepth < 1) {
            return {
              error: `${label} is not a valid dereference.`,
              kind: "compile",
            };
          }
          const ptrRaw = rhs.value;
          if (requireValue && String(ptrRaw ?? "") === "") {
            const sourceLabel = rhs.label || "That pointer";
            return {
              error: `${sourceLabel} doesn't have a value yet, so it can't be dereferenced.`,
              kind: "ub",
            };
          }
          const ptrVal = String(ptrRaw ?? "").trim();
          if (ptrVal === "") {
            const sourceLabel = rhs.label || "That pointer";
            return {
              error: `${sourceLabel} doesn't have a value yet, so it can't be dereferenced.`,
              kind: "ub",
            };
          }
          const target = state.find(
            (b) => String(b.address ?? "") === String(ptrVal),
          );
          if (!target) {
            return {
              error: `${label} doesn't point to a known variable.`,
              kind: "ub",
            };
          }
          return makeLvalue(target, label);
        }
        const scalar = coerceScalarResult(rhs, requireValue);
        if (isScalarError(scalar)) return scalar;
        if (node.op === "~") {
          if (scalar.base === "double") {
            return {
              error: "Bitwise operators require integer values.",
              kind: "compile",
            };
          }
          const value = scalar.value as bigint;
          const overflow = checkIntegerRange(value, scalar.base);
          if (overflow) return overflow;
          return makeRvalue(~value, scalar.base);
        }
        if (node.op === "+")
          return makeRvalue(scalar.value!, scalar.base, 0, "", scalar.nanSign);
        if (node.op === "-") {
          if (scalar.base === "double" && Number.isNaN(scalar.value)) {
            const flipped = scalar.nanSign === -1 ? 1 : -1;
            return makeRvalue(scalar.value!, scalar.base, 0, "", flipped);
          }
          if (scalar.base !== "double") {
            const value = scalar.value as bigint;
            const range = integerRangeForBase(scalar.base);
            if (range && value === range.min) {
              return integerOverflowError(scalar.base);
            }
            return makeRvalue(-value, scalar.base);
          }
          return makeRvalue(-scalar.value!, scalar.base);
        }
        return {
          error: "That expression is not valid here.",
          kind: "compile",
        };
      }
      if (node.kind === "binary") {
        const left = evalNode(node.left);
        if (isEvalError(left)) return left;
        const right = evalNode(node.right);
        if (isEvalError(right)) return right;
        const leftScalar = coerceScalarResult(left, requireValue);
        if (isScalarError(leftScalar)) return leftScalar;
        const rightScalar = coerceScalarResult(right, requireValue);
        if (isScalarError(rightScalar)) return rightScalar;
        const leftValue = leftScalar.value;
        const rightValue = rightScalar.value;
        const useDouble =
          leftScalar.base === "double" || rightScalar.base === "double";
        if (
          node.op === "==" ||
          node.op === "!=" ||
          node.op === "<" ||
          node.op === "<=" ||
          node.op === ">" ||
          node.op === ">="
        ) {
          let result = false;
          if (useDouble) {
            const leftNum = toNumber(leftValue);
            const rightNum = toNumber(rightValue);
            if (Number.isNaN(leftNum) || Number.isNaN(rightNum)) {
              result = node.op === "!=";
            } else if (node.op === "==") {
              result = leftNum === rightNum;
            } else if (node.op === "!=") {
              result = leftNum !== rightNum;
            } else if (node.op === "<") {
              result = leftNum < rightNum;
            } else if (node.op === "<=") {
              result = leftNum <= rightNum;
            } else if (node.op === ">") {
              result = leftNum > rightNum;
            } else if (node.op === ">=") {
              result = leftNum >= rightNum;
            }
          } else {
            const leftBig = leftValue as bigint;
            const rightBig = rightValue as bigint;
            if (node.op === "==") result = leftBig === rightBig;
            else if (node.op === "!=") result = leftBig !== rightBig;
            else if (node.op === "<") result = leftBig < rightBig;
            else if (node.op === "<=") result = leftBig <= rightBig;
            else if (node.op === ">") result = leftBig > rightBig;
            else if (node.op === ">=") result = leftBig >= rightBig;
          }
          return makeRvalue(
            result ? 1n : 0n,
            "int",
          );
        }
        if (
          node.op === "<<" ||
          node.op === ">>" ||
          node.op === "&" ||
          node.op === "^" ||
          node.op === "|"
        ) {
          if (useDouble) {
            return {
              error: "Bitwise operators require integer values.",
              kind: "compile",
            };
          }
          const leftBig = leftValue as bigint;
          const rightBig = rightValue as bigint;
          const base =
            leftScalar.base === "long" || rightScalar.base === "long"
              ? "long"
              : "int";
          const width = bitWidthForBase(base);
          const leftOverflow = checkIntegerRange(leftBig, base);
          if (leftOverflow) return leftOverflow;
          const rightOverflow = checkIntegerRange(rightBig, base);
          if (rightOverflow) return rightOverflow;
          if (node.op === "&") {
            return makeRvalue(leftBig & rightBig, base);
          }
          if (node.op === "^") {
            return makeRvalue(leftBig ^ rightBig, base);
          }
          if (node.op === "|") {
            return makeRvalue(leftBig | rightBig, base);
          }
          if (width == null || rightBig < 0n || rightBig >= BigInt(width)) {
            return { error: "That shift is undefined.", kind: "ub" };
          }
          if (node.op === "<<" && leftBig < 0n) {
            return { error: "That shift is undefined.", kind: "ub" };
          }
          const shifted =
            node.op === "<<" ? leftBig << rightBig : leftBig >> rightBig;
          const overflow = checkIntegerRange(shifted, base);
          if (overflow) return overflow;
          return makeRvalue(shifted, base);
        }
        if (node.op === "/" && !useDouble && rightValue === 0n)
          return { error: "Division by 0 is undefined.", kind: "ub" };
        let value: bigint | number;
        if (useDouble) {
          const leftNum = toNumber(leftValue);
          const rightNum = toNumber(rightValue);
          if (Number.isNaN(leftNum) || Number.isNaN(rightNum)) {
            const nanSign = Number.isNaN(leftNum)
              ? leftScalar.nanSign
              : rightScalar.nanSign;
            return makeRvalue(NaN, "double", 0, "", nanSign);
          }
          if (node.op === "+") value = leftNum + rightNum;
          else if (node.op === "-") value = leftNum - rightNum;
          else if (node.op === "*") value = leftNum * rightNum;
          else if (node.op === "/") value = leftNum / rightNum;
          else
            return {
              error: "That expression is not valid here.",
              kind: "compile",
            };
        } else {
          const leftBig = leftValue as bigint;
          const rightBig = rightValue as bigint;
          const base =
            leftScalar.base === "long" || rightScalar.base === "long"
              ? "long"
              : "int";
          if (node.op === "/" && rightBig === -1n) {
            const range = integerRangeForBase(base);
            if (range && leftBig === range.min) {
              return integerOverflowError(base);
            }
          }
          if (node.op === "+") value = leftBig + rightBig;
          else if (node.op === "-") value = leftBig - rightBig;
          else if (node.op === "*") value = leftBig * rightBig;
          else if (node.op === "/") value = leftBig / rightBig;
          else
            return {
              error: "That expression is not valid here.",
              kind: "compile",
            };
          const overflow = checkIntegerRange(value as bigint, base);
          if (overflow) return overflow;
        }
        const base = useDouble
          ? "double"
          : leftScalar.base === "long" || rightScalar.base === "long"
            ? "long"
            : "int";
        return makeRvalue(value, base);
      }
      return { error: "That expression is not valid here.", kind: "compile" };
    }

    return evalNode(expr);
  }

  function evaluateExpression(
    expr: ExprNode,
    state: BoxState[],
    opts: {
      allowVars?: boolean;
      targetType?: string;
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
    const evaluated = evaluateExpression(expr, state, {
      allowVars: true,
      targetType: "double",
    });
    if (isScalarError(evaluated)) return evaluated;
    const base = evaluated.base || "int";
    if (base === "double") {
      const num =
        typeof evaluated.value === "number"
          ? evaluated.value
          : Number(evaluated.value);
      return { value: num !== 0 };
    }
    const value = evaluated.value as bigint;
    return { value: value !== 0n };
  }

  function evaluateExpressionText(
    expr: string,
    state: BoxState[],
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
        error: string | { text: string; html: string };
        kind: "compile" | "ub";
      } {
    const tokens = tokenizeProgram(expr || "");
    if (!tokens.length) {
      return { error: "Enter an expression to evaluate.", kind: "compile" };
    }
    if (tokens.some((t) => t.type === "unknown")) {
      return {
        error:
          "That line has a character that does not belong in an expression.",
        kind: "compile",
      };
    }
    if (tokens.some((t) => t.type === "sym" && t.value === ";")) {
      return { error: "That expression is not valid here.", kind: "compile" };
    }
    const mapped = tokens.map((tok) =>
      tok.type === "kw" ? { ...tok, type: "ident" as const } : tok,
    );
    const parsed = parseExpressionTokens(mapped, 0, { allowVars: true });
    if (!parsed || parsed.nextIndex !== mapped.length) {
      return { error: "That expression is not valid here.", kind: "compile" };
    }
    const evaluated = evaluateExpressionRaw(parsed.expr, state, {
      allowVars: true,
      targetType: "double",
    });
    if (isEvalError(evaluated)) return evaluated;
    const resultType = makePointerType(
      Number.isFinite(evaluated.depth) ? evaluated.depth : 0,
      evaluated.base || "int",
    );
    return {
      result: {
        kind: evaluated.kind || "rvalue",
        type: resultType || "int",
        value: evaluated.value,
        address: evaluated.kind === "lvalue" ? evaluated.address : "",
        nanSign: evaluated.nanSign,
      },
    };
  }

  function buildUnaryExpr(ops: string[], name: string): ExprNode {
    let expr: ExprNode = { kind: "var", name };
    for (let i = ops.length - 1; i >= 0; i--) {
      const op = ops[i] as "+" | "-" | "*" | "&";
      expr = { kind: "unary", op, expr };
    }
    return expr;
  }

  function convertScalarForAssignment(
    value: bigint | number,
    base: string,
    targetType: string,
    nanSign?: -1 | 1,
  ): { value: bigint | number; base: string; nanSign?: -1 | 1 } | null {
    const { base: targetBase, depth } = parseType(targetType);
    if (!targetBase || depth !== 0) return null;
    if (targetBase === "double") {
      const num =
        base === "double"
          ? typeof value === "bigint"
            ? Number(value)
            : value
          : Number(value);
      const nextNanSign = Number.isNaN(num) ? (nanSign ?? 1) : undefined;
      return { value: num, base: "double", nanSign: nextNanSign };
    }
    let numValue: number;
    if (base === "double") {
      const num = typeof value === "bigint" ? Number(value) : value;
      if (!Number.isFinite(num)) return null;
      numValue = Math.trunc(num);
    } else if (typeof value === "bigint") {
      return { value, base: targetBase };
    } else {
      numValue = Math.trunc(value);
    }
    try {
      return { value: BigInt(numValue), base: targetBase };
    } catch {
      return null;
    }
  }

  function assignScalarFromExpr(
    boxes: BoxState[],
    targetName: string,
    targetType: string,
    expr: ExprNode,
    allowVars: boolean = allowVarAssign,
  ): BoxState[] | null {
    const { depth } = parseType(targetType);
    if (Number.isFinite(depth) && depth > 0) return null;
    const evaluated = evaluateExpression(expr, boxes, {
      allowVars,
      targetType,
    });
    if (isScalarError(evaluated)) return null;
    const converted = convertScalarForAssignment(
      evaluated.value!,
      evaluated.base,
      targetType,
      evaluated.nanSign,
    );
    if (!converted) return null;
    const by = Object.fromEntries(boxes.map((b) => [b.name, b]));
    const target = by[targetName];
    if (!target) return null;
    target.value = formatValueForType(converted.value, targetType, {
      nanSign: converted.nanSign,
    });
    return boxes;
  }

  function parseAssignRhs(
    tokens: Token[],
    idx: number,
    { allowVar }: { allowVar?: boolean } = {},
  ): AssignRhs | null {
    if (idx >= tokens.length) return null;
    const allowVars = allowVar ?? allowVarAssign;
    const rhs = tokens[idx];
    if (rhs.type === "number" && idx === tokens.length - 1) {
      return { kind: "num", value: rhs.value };
    }
    if (rhs.type === "ident" && allowVars && idx === tokens.length - 1) {
      return { kind: "var", name: rhs.value };
    }
    if (rhs.type === "sym" && rhs.value === "&" && allowPointers) {
      const next = tokens[idx + 1];
      if (next?.type === "ident" && idx + 2 === tokens.length) {
        return { kind: "ref", name: next.value };
      }
    }
    if (
      rhs.type === "sym" &&
      (rhs.value === "*" || rhs.value === "&") &&
      allowPointers
    ) {
      let j = idx;
      let depth = 0;
      while (
        j < tokens.length &&
        tokens[j].type === "sym" &&
        (tokens[j].value === "*" || tokens[j].value === "&")
      ) {
        depth++;
        j++;
      }
      if (depth > 0 && tokens[j]?.type === "ident" && j + 1 === tokens.length) {
        const ops = tokens.slice(idx, j).map((tok) => String(tok.value));
        if (ops.every((op) => op === "*")) {
          return { kind: "deref", name: tokens[j].value, depth };
        }
        return { kind: "unary", name: tokens[j].value, ops };
      }
    }
    const parsed = parseExpressionTokens(tokens, idx, { allowVars });
    if (parsed && parsed.nextIndex === tokens.length) {
      return { kind: "expr", expr: parsed.expr, hasVar: parsed.hasVar };
    }
    return null;
  }

  function parseUnaryLhs(tokens: Token[]): UnaryLhs | null {
    if (!allowPointers) return null;
    if (!tokens.length) return null;
    let idx = 0;
    const ops: string[] = [];
    while (
      idx < tokens.length &&
      tokens[idx].type === "sym" &&
      (tokens[idx].value === "*" || tokens[idx].value === "&")
    ) {
      ops.push(tokens[idx].value);
      idx++;
    }
    if (!ops.length) return null;
    if (idx >= tokens.length || tokens[idx].type !== "ident") return null;
    const name = tokens[idx].value;
    return { ops, name, idx: idx + 1 };
  }

  function parseIfHeaderTokens(
    tokens: Token[],
  ): { expr: ExprNode; hasVar: boolean } | null {
    if (!tokens.length) return null;
    if (tokens[0].type !== "kw" || tokens[0].value !== "if") return null;
    if (tokens.length < 3) return null;
    if (tokens[1].type !== "sym" || tokens[1].value !== "(") return null;
    let depth = 0;
    let endIdx = -1;
    for (let i = 1; i < tokens.length; i++) {
      const tok = tokens[i];
      if (tok.type === "sym" && tok.value === "(") {
        depth++;
        continue;
      }
      if (tok.type === "sym" && tok.value === ")") {
        depth--;
        if (depth === 0) {
          endIdx = i;
          break;
        }
      }
    }
    if (endIdx < 0 || endIdx !== tokens.length - 1) return null;
    const exprTokens = tokens.slice(2, endIdx);
    if (!exprTokens.length) return null;
    const parsed = parseExpressionTokens(exprTokens, 0, { allowVars: true });
    if (!parsed || parsed.nextIndex !== exprTokens.length) return null;
    return { expr: parsed.expr, hasVar: parsed.hasVar };
  }

  function parseStatementTokens(tokens: Token[]): Statement | null {
    if (!tokens.length) return null;
    if (tokens.length === 1 && tokens[0].type === "sym") {
      if (tokens[0].value === "{") return { kind: "blockStart" };
      if (tokens[0].value === "}") return { kind: "blockEnd" };
    }
    const ifParsed = parseIfHeaderTokens(tokens);
    if (ifParsed) {
      return { kind: "if", expr: ifParsed.expr, hasVar: ifParsed.hasVar };
    }
    if (tokens[0].type === "kw") {
      const baseType = tokens[0].value;
      if (baseType !== "int" && baseType !== "long" && baseType !== "double")
        return null;
      let idx = 1;
      let stars = 0;
      while (
        idx < tokens.length &&
        tokens[idx].type === "sym" &&
        tokens[idx].value === "*"
      ) {
        if (!allowPointers) return null;
        stars++;
        idx++;
      }
      const declType = resolveDeclType(stars, baseType);
      if (!declType) return null;
      if (idx >= tokens.length || tokens[idx].type !== "ident") return null;
      const name = tokens[idx].value;
      idx++;
      if (idx === tokens.length) {
        return { kind: "decl", name, type: declType };
      }
      if (tokens[idx].type !== "sym" || tokens[idx].value !== "=") return null;
      idx++;
      if (idx >= tokens.length) return null;
      const rhs = parseAssignRhs(tokens, idx, {
        allowVar: allowVarAssign,
      });
      if (!rhs) return null;
      if (rhs.kind === "num") {
        return {
          kind: "declAssign",
          name,
          value: rhs.value,
          valueKind: "num",
          declType,
        };
      }
      if (rhs.kind === "var") {
        return { kind: "declAssignVar", name, src: rhs.name, declType };
      }
      if (rhs.kind === "expr") {
        return {
          kind: "declAssign",
          name,
          valueKind: "expr",
          expr: rhs.expr,
          hasVar: rhs.hasVar,
          declType,
        };
      }
      if (rhs.kind === "ref") {
        if (!allowPointers) return null;
        if (!isPointerType(declType)) return null;
        return { kind: "declAssignRef", name, ref: rhs.name, declType };
      }
      if (rhs.kind === "deref") {
        if (!allowPointers) return null;
        return {
          kind: "declAssignDeref",
          name,
          ptr: rhs.name,
          depth: rhs.depth,
          declType,
        };
      }
      if (rhs.kind === "unary") {
        if (!allowPointers) return null;
        return {
          kind: "declAssignUnary",
          name,
          src: rhs.name,
          ops: rhs.ops,
          declType,
        };
      }
      return null;
    }
    const unary = parseUnaryLhs(tokens);
    if (unary) {
      let idx = unary.idx;
      if (
        idx >= tokens.length ||
        tokens[idx].type !== "sym" ||
        tokens[idx].value !== "="
      )
        return null;
      idx++;
      const rhs = parseAssignRhs(tokens, idx, { allowVar: allowVarAssign });
      if (!rhs) return null;
      return { kind: "assignUnary", name: unary.name, ops: unary.ops, rhs };
    }
    if (
      tokens.length >= 3 &&
      tokens[0].type === "ident" &&
      tokens[1].type === "sym" &&
      tokens[1].value === "="
    ) {
      const rhs = parseAssignRhs(tokens, 2, { allowVar: allowVarAssign });
      if (!rhs) return null;
      if (rhs.kind === "num") {
        return {
          kind: "assign",
          name: tokens[0].value,
          value: rhs.value,
          valueKind: "num",
        };
      }
      if (rhs.kind === "var") {
        return { kind: "assignVar", name: tokens[0].value, src: rhs.name };
      }
      if (rhs.kind === "expr") {
        return {
          kind: "assign",
          name: tokens[0].value,
          valueKind: "expr",
          expr: rhs.expr,
          hasVar: rhs.hasVar,
        };
      }
      if (rhs.kind === "ref") {
        return { kind: "assignRef", name: tokens[0].value, ref: rhs.name };
      }
      if (rhs.kind === "deref") {
        return {
          kind: "assignFromDeref",
          name: tokens[0].value,
          ptr: rhs.name,
          depth: rhs.depth,
        };
      }
      if (rhs.kind === "unary") {
        return {
          kind: "assignUnaryRhs",
          name: tokens[0].value,
          src: rhs.name,
          ops: rhs.ops,
        };
      }
      return null;
    }
    return null;
  }

  function applyAssignVar(
    state: BoxState[],
    stmt: Extract<Statement, { kind: "assignVar" | "declAssignVar" }>,
  ): BoxState[] | null {
    const boxes = cloneBoxes(state);
    const by = Object.fromEntries(boxes.map((b) => [b.name, b]));
    const target = by[stmt.name];
    const source = by[stmt.src];
    if (!target || !source) return null;
    const { base: targetBase, depth: targetDepth } = parseType(target.type);
    const { base: sourceBase, depth: sourceDepth } = parseType(source.type);
    const sameType = target.type === source.type;
    const isScalar =
      targetBase &&
      sourceBase &&
      targetBase === sourceBase &&
      targetDepth === 0 &&
      sourceDepth === 0;
    const isPtr = sameType && isPointerType(target.type);
    if (targetDepth === 0 && sourceDepth === 0) {
      return assignScalarFromExpr(boxes, stmt.name, target.type, {
        kind: "var",
        name: stmt.src,
      });
    }
    if (!isScalar && !isPtr) return null;
    if (requireSourceValue && String(source.value ?? "") === "") return null;
    target.value = String(source.value ?? "");
    return boxes;
  }

  function applyAssignExpr(
    state: BoxState[],
    stmt: Extract<
      Statement,
      | { kind: "assign"; valueKind: "expr" }
      | { kind: "declAssign"; valueKind: "expr" }
    >,
    { allowVars = allowVarAssign }: { allowVars?: boolean } = {},
  ): BoxState[] | null {
    const boxes = cloneBoxes(state);
    const by = Object.fromEntries(boxes.map((b) => [b.name, b]));
    const target = by[stmt.name];
    if (!target) return null;
    const { depth } = parseType(target.type);
    if (Number.isFinite(depth) && depth > 0) return null;
    return assignScalarFromExpr(
      boxes,
      stmt.name,
      target.type,
      stmt.expr as ExprNode,
      allowVars,
    );
  }

  function applyAssignRef(
    state: BoxState[],
    stmt: Extract<Statement, { kind: "assignRef" | "declAssignRef" }>,
  ): BoxState[] | null {
    const boxes = cloneBoxes(state);
    const by = Object.fromEntries(boxes.map((b) => [b.name, b]));
    const target = by[stmt.name];
    const refBox = by[stmt.ref];
    if (!target || !refBox || !refBox.address) return null;
    if (!isRefCompatible(target.type, refBox.type)) return null;
    target.value = String(refBox.address);
    return boxes;
  }

  function resolveDerefTarget(
    state: BoxState[],
    ptrName: string,
    depth: number,
  ): { target?: BoxState; error?: string } {
    const by = Object.fromEntries(state.map((b) => [b.name, b]));
    const ptr = by[ptrName];
    if (!ptr) return { error: "missing" };
    let current = ptr;
    for (let i = 0; i < depth; i++) {
      if (!isPointerType(current.type)) return { error: "type" };
      if (String(current.value ?? "") === "") return { error: "empty" };
      const next = state.find((b) => b.address === String(current.value));
      if (!next) return { error: "unknown" };
      current = next;
    }
    return { target: current };
  }

  function applyAssignDeref(
    state: BoxState[],
    stmt: Extract<
      Statement,
      { kind: "assignDeref" | "assignDerefVar" | "assignDerefRef" }
    >,
  ): BoxState[] | null {
    const boxes = cloneBoxes(state);
    const { target } = resolveDerefTarget(boxes, stmt.name, stmt.depth || 1);
    if (!target) return null;
    const { base: targetBase, depth: targetDepth } = parseType(target.type);
    if (!Number.isFinite(targetDepth)) return null;
    if (stmt.kind === "assignDeref") {
      if (targetDepth !== 0) return null;
      return assignScalarFromExpr(boxes, target.name, target.type, {
        kind: "num",
        value: stmt.value,
      });
    } else if (stmt.kind === "assignDerefVar") {
      if (targetDepth === 0) {
        return assignScalarFromExpr(boxes, target.name, target.type, {
          kind: "var",
          name: stmt.src,
        });
      }
      const by = Object.fromEntries(boxes.map((b) => [b.name, b]));
      const source = by[stmt.src];
      if (!source) return null;
      if (requireSourceValue && String(source.value ?? "") === "") return null;
      const { base: sourceBase, depth: sourceDepth } = parseType(source.type);
      if (
        !sourceBase ||
        !targetBase ||
        sourceBase !== targetBase ||
        sourceDepth !== targetDepth
      )
        return null;
      target.value = String(source.value ?? "");
    } else if (stmt.kind === "assignDerefRef") {
      if (targetDepth === 0) return null;
      const by = Object.fromEntries(boxes.map((b) => [b.name, b]));
      const refBox = by[stmt.ref];
      if (!refBox || !refBox.address) return null;
      if (!isRefCompatible(target.type, refBox.type)) return null;
      target.value = String(refBox.address);
    }
    return boxes;
  }

  function resolveUnaryLvalue(
    state: BoxState[],
    ops: string[],
    name: string,
  ): {
    target?: BoxState;
    label?: string;
    type?: string;
    error?: string;
    name?: string;
  } {
    const by = Object.fromEntries(state.map((b) => [b.name, b]));
    const base = by[name];
    if (!base) return { error: "missing", name };
    const nanSignForType = (
      value: BoxValue,
      type: string,
    ): -1 | 1 | undefined => {
      const { base } = parseType(type);
      if (base !== "double") return undefined;
      const parsed = parseDoubleValueWithSign(value);
      return parsed?.nanSign;
    };
    let current: UnaryResult = {
      kind: "lvalue",
      type: base.type,
      value: base.value,
      address: base.address ?? "",
      box: base,
      nanSign: nanSignForType(base.value, base.type),
    };
    let label = name;
    for (let i = ops.length - 1; i >= 0; i--) {
      const op = ops[i];
      if (op === "&") {
        const nextLabel = `&${label}`;
        if (current.kind !== "lvalue" || !current.address) {
          return { error: "not_lvalue", label: nextLabel };
        }
        const { base, depth } = parseType(current.type);
        const nextDepth = Number.isFinite(depth) ? depth + 1 : 1;
        current = {
          kind: "rvalue",
          type: makePointerType(nextDepth, base || "int") || "int*",
          value: String(current.address),
          address: "",
          box: null,
          nanSign: undefined,
        };
        label = nextLabel;
        continue;
      }
      if (op === "*") {
        const nextLabel = `*${label}`;
        const { base, depth } = parseType(current.type);
        if (!Number.isFinite(depth) || depth < 1) {
          return { error: "not_deref", label: nextLabel };
        }
        const ptrVal: string = String(current.value ?? "").trim();
        if (ptrVal === "") {
          return { error: "empty", label };
        }
        const target: BoxState | undefined = state.find(
          (b) => String(b.address ?? "") === String(ptrVal),
        );
        if (!target) return { error: "unknown", label: nextLabel };
        current = {
          kind: "lvalue",
          type: makePointerType(depth - 1, base || "int") || "int",
          value: target.value,
          address: target.address ?? "",
          box: target,
          nanSign: nanSignForType(target.value, target.type),
        };
        label = nextLabel;
      }
    }
    if (current.kind !== "lvalue" || !current.box) {
      return { error: "not_lvalue", label };
    }
    return { target: current.box, label, type: current.type };
  }

  function resolveUnaryExpr(
    state: BoxState[],
    ops: string[],
    name: string,
  ): { result?: UnaryResult; label?: string; error?: string; name?: string } {
    const by = Object.fromEntries(state.map((b) => [b.name, b]));
    const base = by[name];
    if (!base) return { error: "missing", name };
    const nanSignForType = (
      value: BoxValue,
      type: string,
    ): -1 | 1 | undefined => {
      const { base } = parseType(type);
      if (base !== "double") return undefined;
      const parsed = parseDoubleValueWithSign(value);
      return parsed?.nanSign;
    };
    let current: UnaryResult = {
      kind: "lvalue",
      type: base.type,
      value: base.value,
      address: base.address ?? "",
      box: base,
      refBox: null,
      nanSign: nanSignForType(base.value, base.type),
    };
    let label = name;
    for (let i = ops.length - 1; i >= 0; i--) {
      const op = ops[i];
      if (op === "&") {
        const nextLabel = `&${label}`;
        if (current.kind !== "lvalue" || !current.address) {
          return { error: "not_lvalue", label: nextLabel };
        }
        const { base, depth } = parseType(current.type);
        const nextDepth = Number.isFinite(depth) ? depth + 1 : 1;
        current = {
          kind: "rvalue",
          type: makePointerType(nextDepth, base || "int") || "int*",
          value: String(current.address),
          address: "",
          box: null,
          refBox: current.box,
          nanSign: undefined,
        };
        label = nextLabel;
        continue;
      }
      if (op === "*") {
        const nextLabel = `*${label}`;
        const { base, depth } = parseType(current.type);
        if (!Number.isFinite(depth) || depth < 1) {
          return { error: "not_deref", label: nextLabel };
        }
        const ptrVal: string = String(current.value ?? "").trim();
        if (ptrVal === "") {
          return { error: "empty", label };
        }
        const target: BoxState | undefined = state.find(
          (b) => String(b.address ?? "") === String(ptrVal),
        );
        if (!target) return { error: "unknown", label: nextLabel };
        current = {
          kind: "lvalue",
          type: makePointerType(depth - 1, base || "int") || "int",
          value: target.value,
          address: target.address ?? "",
          box: target,
          refBox: null,
          nanSign: nanSignForType(target.value, target.type),
        };
        label = nextLabel;
      }
    }
    return { result: current, label };
  }

  function minBaseDepthForOps(ops: string[]): number {
    let delta = 0;
    let required = 0;
    for (let i = ops.length - 1; i >= 0; i--) {
      const op = ops[i];
      if (op === "&") {
        delta += 1;
      } else if (op === "*") {
        required = Math.max(required, 1 - delta);
        delta -= 1;
      }
    }
    return Math.max(0, required);
  }

  function validateUnaryRhs(
    state: BoxState[],
    targetType: string,
    targetName: string,
    ops: string[],
    src: string,
  ): EvalError | null {
    const by = Object.fromEntries(state.map((b) => [b.name, b]));
    const base = by[src];
    const minDepth = minBaseDepthForOps(ops || []);
    const { base: srcBase } = parseType(base?.type);
    const requiredBaseType =
      makePointerType(minDepth, srcBase || "int") ||
      `int${"*".repeat(minDepth)}`;
    if (!base) {
      return {
        error: `You can't use ${src} before declaring it.`,
        kind: "compile",
      };
    }
    const baseDepth = pointerDepth(base.type);
    if (
      baseDepth == null ||
      !Number.isFinite(baseDepth) ||
      baseDepth < minDepth
    ) {
      return typeMismatchError(src, requiredBaseType);
    }
    const resolved = resolveUnaryExpr(state, ops, src);
    if (resolved?.error === "empty") {
      return {
        error: `${resolved.label} doesn't have a value yet, so it can't be dereferenced.`,
        kind: "ub",
      };
    }
    if (resolved?.error === "unknown") {
      return {
        error: `${resolved.label} doesn't point to a known variable.`,
        kind: "ub",
      };
    }
    if (resolved?.error === "not_deref") {
      return {
        error: `${resolved.label} is not a valid dereference.`,
        kind: "compile",
      };
    }
    if (resolved?.error === "not_lvalue") {
      return { error: "That assignment is not valid here.", kind: "compile" };
    }
    const result = resolved?.result;
    if (!result) {
      return { error: "That assignment is not valid here.", kind: "compile" };
    }
    const { base: targetBase, depth: targetDepth } = parseType(targetType);
    const { base: resultBase, depth: resultDepth } = parseType(result.type);
    if (
      !targetBase ||
      !resultBase ||
      !Number.isFinite(targetDepth) ||
      !Number.isFinite(resultDepth)
    ) {
      return { error: "That assignment is not valid here.", kind: "compile" };
    }
    if (targetBase !== resultBase || targetDepth !== resultDepth) {
      return typeMismatchError(
        targetName,
        makePointerType(resultDepth, resultBase) ||
          `int${"*".repeat(resultDepth)}`,
      );
    }
    if (result.kind === "lvalue") {
      if (requireSourceValue && String(result.value ?? "") === "") {
        return {
          error: `${resolved.label} doesn't have a value yet.`,
          kind: "ub",
        };
      }
    }
    return null;
  }

  function applyAssignUnary(
    state: BoxState[],
    stmt: Extract<Statement, { kind: "assignUnary" }>,
  ): BoxState[] | null {
    const boxes = cloneBoxes(state);
    const resolved = resolveUnaryLvalue(boxes, stmt.ops, stmt.name);
    if (!resolved?.target) return null;
    const targetName = resolved.target.name;
    if (stmt.rhs.kind === "num") {
      return applyStatement(
        boxes,
        {
          kind: "assign",
          name: targetName,
          value: stmt.rhs.value,
          valueKind: "num",
        },
        {},
      );
    }
    if (stmt.rhs.kind === "var") {
      return applyAssignVar(boxes, {
        kind: "assignVar",
        name: targetName,
        src: stmt.rhs.name,
      });
    }
    if (stmt.rhs.kind === "ref") {
      return applyAssignRef(boxes, {
        kind: "assignRef",
        name: targetName,
        ref: stmt.rhs.name,
      });
    }
    if (stmt.rhs.kind === "deref") {
      return applyAssignFromDeref(boxes, {
        kind: "assignFromDeref",
        name: targetName,
        ptr: stmt.rhs.name,
        depth: stmt.rhs.depth,
      });
    }
    if (stmt.rhs.kind === "unary") {
      return applyAssignUnaryRhs(boxes, {
        kind: "assignUnaryRhs",
        name: targetName,
        src: stmt.rhs.name,
        ops: stmt.rhs.ops,
      });
    }
    if (stmt.rhs.kind === "expr") {
      const targetType = resolved.target?.type || "int";
      return assignScalarFromExpr(boxes, targetName, targetType, stmt.rhs.expr);
    }
    return null;
  }

  function applyAssignUnaryRhs(
    state: BoxState[],
    stmt: Extract<Statement, { kind: "assignUnaryRhs" | "declAssignUnary" }>,
  ): BoxState[] | null {
    const boxes = cloneBoxes(state);
    const by = Object.fromEntries(boxes.map((b) => [b.name, b]));
    const target = by[stmt.name];
    if (!target) return null;
    const { depth: targetDepth } = parseType(target.type);
    if (targetDepth === 0) {
      const expr = buildUnaryExpr(stmt.ops || [], stmt.src);
      return assignScalarFromExpr(boxes, stmt.name, target.type, expr);
    }
    const resolved = resolveUnaryExpr(boxes, stmt.ops, stmt.src);
    if (!resolved?.result) return null;
    const result = resolved.result;
    const { base: targetBase } = parseType(target.type);
    const { base: resultBase, depth: resultDepth } = parseType(result.type);
    if (
      !targetBase ||
      !resultBase ||
      !Number.isFinite(targetDepth) ||
      !Number.isFinite(resultDepth)
    )
      return null;
    if (targetBase !== resultBase || targetDepth !== resultDepth) return null;
    const value = result.kind === "lvalue" ? result.box?.value : result.value;
    if (requireSourceValue && String(value ?? "") === "") return null;
    target.value = formatValueForType(value ?? "", target.type, {
      nanSign: result.nanSign,
    });
    return boxes;
  }

  function applyAssignFromDeref(
    state: BoxState[],
    stmt: Extract<Statement, { kind: "assignFromDeref" | "declAssignDeref" }>,
  ): BoxState[] | null {
    const boxes = cloneBoxes(state);
    const by = Object.fromEntries(boxes.map((b) => [b.name, b]));
    const target = by[stmt.name];
    if (!target) return null;
    const { depth: targetDepth } = parseType(target.type);
    if (targetDepth === 0) {
      const expr = buildUnaryExpr(
        new Array(stmt.depth || 1).fill("*"),
        stmt.ptr,
      );
      return assignScalarFromExpr(boxes, stmt.name, target.type, expr);
    }
    const { target: source } = resolveDerefTarget(
      boxes,
      stmt.ptr,
      stmt.depth || 1,
    );
    if (!source) return null;
    const { base: targetBase } = parseType(target.type);
    const { base: sourceBase, depth: sourceDepth } = parseType(source.type);
    if (
      !targetBase ||
      !sourceBase ||
      !Number.isFinite(targetDepth) ||
      !Number.isFinite(sourceDepth)
    )
      return null;
    if (targetBase !== sourceBase || targetDepth !== sourceDepth) return null;
    if (requireSourceValue && String(source.value ?? "") === "") return null;
    target.value = String(source.value ?? "");
    return boxes;
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
      if (by[stmt.name] && !allowRedeclare) return null;
      if (!by[stmt.name]) {
        boxes.push({
          name: stmt.name,
          type,
          value: "",
          address: alloc(type),
        });
      }
      return boxes;
    }
    if (stmt.kind === "assign" && stmt.valueKind === "num") {
      const target = by[stmt.name];
      if (!target) return null;
      const { base, depth } = parseType(target.type);
      if (!base || depth !== 0) return null;
      return assignScalarFromExpr(boxes, stmt.name, target.type, {
        kind: "num",
        value: stmt.value,
      });
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
        });
      }
      if (stmt.valueKind === "expr") {
        return applyAssignExpr(boxes, stmt, { allowVars: allowVarAssign });
      }
      return assignScalarFromExpr(boxes, stmt.name, declType, {
        kind: "num",
        value: stmt.value,
      });
    }
    if (stmt.kind === "declAssignVar") {
      const declType = stmt.declType || "int";
      if (by[stmt.name] && !allowRedeclare) return null;
      if (!by[stmt.name]) {
        boxes.push({
          name: stmt.name,
          type: declType,
          value: "",
          address: alloc(declType),
        });
      }
      return applyAssignVar(boxes, stmt);
    }
    if (stmt.kind === "declAssignRef") {
      const declType = stmt.declType || "int*";
      if (by[stmt.name] && !allowRedeclare) return null;
      if (!by[stmt.name]) {
        boxes.push({
          name: stmt.name,
          type: declType,
          value: "",
          address: alloc(declType),
        });
      }
      return applyAssignRef(boxes, stmt);
    }
    if (stmt.kind === "declAssignDeref") {
      const declType = stmt.declType || "int";
      if (by[stmt.name] && !allowRedeclare) return null;
      if (!by[stmt.name]) {
        boxes.push({
          name: stmt.name,
          type: declType,
          value: "",
          address: alloc(declType),
        });
      }
      return applyAssignFromDeref(boxes, stmt);
    }
    if (stmt.kind === "declAssignUnary") {
      const declType = stmt.declType || "int";
      if (by[stmt.name] && !allowRedeclare) return null;
      if (!by[stmt.name]) {
        boxes.push({
          name: stmt.name,
          type: declType,
          value: "",
          address: alloc(declType),
        });
      }
      return applyAssignUnaryRhs(boxes, {
        kind: "assignUnaryRhs",
        name: stmt.name,
        src: stmt.src,
        ops: stmt.ops,
      });
    }
    if (stmt.kind === "assignVar") {
      return applyAssignVar(state, stmt);
    }
    if (stmt.kind === "assign" && stmt.valueKind === "expr") {
      return applyAssignExpr(state, stmt, { allowVars: allowVarAssign });
    }
    if (stmt.kind === "assignRef") {
      return applyAssignRef(state, stmt);
    }
    if (stmt.kind === "assignUnary") {
      return applyAssignUnary(state, stmt);
    }
    if (stmt.kind === "assignUnaryRhs") {
      return applyAssignUnaryRhs(state, stmt);
    }
    if (stmt.kind === "assignFromDeref") {
      return applyAssignFromDeref(state, stmt);
    }
    if (
      stmt.kind === "assignDeref" ||
      stmt.kind === "assignDerefVar" ||
      stmt.kind === "assignDerefRef"
    ) {
      return applyAssignDeref(state, stmt);
    }
    return null;
  }

  function missingDeclError(
    name: string,
    typeLabel: string = "int",
  ): EvalError {
    const text = `You can't assign to ${name} before declaring it. You need to first declare it (${typeLabel} ${name};) prior to this line.`;
    const html = `You can't assign to <code class="tok-name">${name}</code> before declaring it. You need to first declare it (<code class="tok-code">${typeLabel} ${name};</code>) prior to this line.`;
    return { error: { text, html }, kind: "compile" };
  }

  function typeMismatchError(name: string, expectedType: string): EvalError {
    const text = `${name}'s type would need to be ${expectedType} for this line to work.`;
    const html = `<code class="tok-name">${name}</code>'s type would need to be <code class="tok-type">${expectedType}</code> for this line to work.`;
    return { error: { text, html }, kind: "compile" };
  }

  function describeTokensError(
    tokens: Token[],
    seenDecl: DeclaredNames,
  ): string {
    if (!tokens.length) return "Line has an error.";
    if (tokens.some((t) => t.type === "unknown" && t.value === "/*"))
      return "Block comment is not closed.";
    if (tokens.some((t) => t.type === "unknown"))
      return "That line has a character that does not belong in a declaration or assignment.";
    if (tokens[0].type === "kw" && tokens[0].value === "if") {
      return 'If statements should look like "if (condition) { ... }".';
    }
    if (tokens[0].type === "kw" && tokens[0].value === "else") {
      return "Else statements are not supported yet.";
    }
    if (tokens[0].type === "kw") {
      const baseType = tokens[0].value;
      if (baseType !== "int" && baseType !== "long" && baseType !== "double")
        return "A declaration needs a variable name.";
      if (tokens.length === 1) return "A declaration needs a variable name.";
      if (
        tokens.length >= 2 &&
        tokens[1].type === "sym" &&
        tokens[1].value === "*"
      ) {
        let idx = 1;
        while (
          idx < tokens.length &&
          tokens[idx].type === "sym" &&
          tokens[idx].value === "*"
        )
          idx++;
        if (tokens[idx]?.type !== "ident")
          return "A declaration needs a variable name.";
        if (
          tokens[idx + 1]?.type === "sym" &&
          tokens[idx + 1].value === "=" &&
          tokens[idx + 2]?.type === "number"
        ) {
          return `Pointer declarations should assign from an address, like "${baseType}* name = &x;".`;
        }
        return "A declaration needs a variable name.";
      }
      if (tokens[1].type !== "ident")
        return "A declaration needs a variable name.";
      return 'Declarations should look like "int name;" or "long name;" or "double name;" or "int name = value;".';
    }
    if (allowPointers && tokens[0].type === "sym" && tokens[0].value === "*") {
      return 'Assignments through pointers should look like "*name = value;".';
    }
    if (tokens[0].type === "ident") {
      const name = tokens[0].value;
      if (tokens[1]?.type === "ident") {
        return `${name} isn't a valid type name.`;
      }
      if (!hasDeclaredPrefix(name, seenDecl))
        return `You can't use ${name} before declaring it.`;
      if (tokens.length === 1)
        return 'Assignments should look like "name = value;".';
      if (tokens[1].type !== "sym" || tokens[1].value !== "=")
        return 'Assignments should use "=".';
      if (tokens.length === 2) return "Assignment needs a value on the right.";
      const rhs = tokens[2];
      if (rhs.type === "ident" && !allowVarAssign)
        return "Assignments should use a number.";
      if (rhs.type === "ident" && !hasDeclaredPrefix(rhs.value, seenDecl)) {
        return `You can't use ${rhs.value} before declaring it.`;
      }
      return 'Assignments should look like "name = value;".';
    }
    return "Line should be a declaration or assignment.";
  }

  function validateStatement(
    tokens: Token[],
    state: BoxState[],
    seenDecl: DeclaredNames,
    alloc: (type?: string) => string,
  ): StatementValidationResult {
    if (tokens.some((t) => t.type === "unknown")) {
      return {
        error:
          "That line has a character that does not belong in a declaration or assignment.",
        kind: "compile",
      };
    }
    const parsed = parseStatementTokens(tokens);
    if (!parsed) {
      return {
        error: describeTokensError(tokens, seenDecl),
        kind: "compile",
      };
    }
    if (parsed.kind === "blockStart" || parsed.kind === "blockEnd") {
      return { parsed, next: state };
    }
    if (parsed.kind === "if") {
      const result = evaluateCondition(parsed.expr, state);
      if ("error" in result) {
        return { error: result.error, kind: result.kind };
      }
      return { parsed, next: state };
    }
    if (
      parsed.kind === "decl" ||
      parsed.kind === "declAssign" ||
      parsed.kind === "declAssignVar" ||
      parsed.kind === "declAssignRef" ||
      parsed.kind === "declAssignDeref" ||
      parsed.kind === "declAssignUnary"
    ) {
      if (seenDecl.has(parsed.name))
        return {
          error: `You already declared ${parsed.name}.`,
          kind: "compile",
        };
      if (parsed.kind === "declAssignVar") {
        const by = Object.fromEntries(state.map((b) => [b.name, b]));
        if (!by[parsed.src]) {
          return {
            error: `You can't use ${parsed.src} before declaring it.`,
            kind: "compile",
          };
        }
        if (requireSourceValue && String(by[parsed.src].value ?? "") === "") {
          return {
            error: `${parsed.src} doesn't have a value yet.`,
            kind: "ub",
          };
        }
      }
      if (parsed.kind === "declAssignRef") {
        const by = Object.fromEntries(state.map((b) => [b.name, b]));
        const refBox = by[parsed.ref];
        if (!refBox) {
          return {
            error: `You can't use ${parsed.ref} before declaring it.`,
            kind: "compile",
          };
        }
        if (!isRefCompatible(parsed.declType || "int*", refBox.type)) {
          const expected = expectedPointerTypeForRef(refBox.type);
          if (expected) return typeMismatchError(parsed.name, expected);
          return {
            error: "That assignment is not valid here.",
            kind: "compile",
          };
        }
      }
      if (parsed.kind === "declAssign" && parsed.valueKind === "num") {
        const err = numericLiteralErrorForType(
          parsed.value,
          parsed.declType || "int",
        );
        if (err) return err;
      }
      if (parsed.kind === "declAssign" && parsed.valueKind === "expr") {
        const { depth } = parseType(parsed.declType || "int");
        if (Number.isFinite(depth) && depth > 0) {
          return {
            error: "Pointer arithmetic is not supported here.",
            kind: "compile",
          };
        }
        const evaluated = evaluateExpression(parsed.expr, state, {
          allowVars: allowVarAssign,
          targetType: parsed.declType || "int",
        });
        if (evaluated.error) return evaluated;
      }
      if (parsed.kind === "declAssignDeref") {
        const by = Object.fromEntries(state.map((b) => [b.name, b]));
        const ptr = by[parsed.ptr];
        if (!ptr) {
          return {
            error: `You can't use ${parsed.ptr} before declaring it.`,
            kind: "compile",
          };
        }
        const depth = parsed.depth || 1;
        const { base: ptrBase, depth: ptrDepth } = parseType(ptr.type);
        const { base: declBase, depth: declDepth } = parseType(
          parsed.declType || "int",
        );
        if (!ptrBase || !declBase) {
          return typeMismatchError(
            parsed.ptr,
            makePointerType(depth) || `int${"*".repeat(depth)}`,
          );
        }
        if (!Number.isFinite(ptrDepth) || ptrDepth < depth) {
          return typeMismatchError(
            parsed.ptr,
            makePointerType(depth, ptrBase) || `int${"*".repeat(depth)}`,
          );
        }
        const resultDepth = ptrDepth - depth;
        if (declBase !== ptrBase || declDepth !== resultDepth) {
          return typeMismatchError(
            parsed.name,
            makePointerType(resultDepth, ptrBase) ||
              `int${"*".repeat(resultDepth)}`,
          );
        }
        let current = ptr;
        const derefLabel = `${"*".repeat(depth)}${parsed.ptr}`;
        for (let i = 0; i < depth; i++) {
          if (!isPointerType(current.type)) {
            return typeMismatchError(
              parsed.ptr,
              makePointerType(depth, ptrBase) || `int${"*".repeat(depth)}`,
            );
          }
          if (String(current.value ?? "") === "") {
            return {
              error: `${derefLabel} doesn't have a value yet.`,
              kind: "ub",
            };
          }
          const next = state.find((b) => b.address === String(current.value));
          if (!next) {
            return {
              error: `${parsed.ptr} doesn't point to a known variable.`,
              kind: "ub",
            };
          }
          current = next;
        }
        if (requireSourceValue && String(current.value ?? "") === "") {
          return {
            error: `${derefLabel} doesn't have a value yet.`,
            kind: "ub",
          };
        }
      }
      if (parsed.kind === "declAssignUnary") {
        const err = validateUnaryRhs(
          state,
          parsed.declType || "int",
          parsed.name,
          parsed.ops,
          parsed.src,
        );
        if (err) return err;
      }
    } else if (parsed.kind === "assign") {
      if (!seenDecl.has(parsed.name)) {
        return missingDeclError(parsed.name, "int");
      }
      const by = Object.fromEntries(state.map((b) => [b.name, b]));
      if (parsed.valueKind === "expr") {
        const target = by[parsed.name];
        const { depth } = parseType(target?.type || "int");
        if (Number.isFinite(depth) && depth > 0) {
          return {
            error: "Pointer arithmetic is not supported here.",
            kind: "compile",
          };
        }
        const evaluated = evaluateExpression(parsed.expr, state, {
          allowVars: allowVarAssign,
          targetType: target?.type || "int",
        });
        if (evaluated.error) return evaluated;
      } else {
        const err = numericLiteralErrorForType(
          parsed.value,
          by[parsed.name]?.type || "int",
        );
        if (err) return err;
      }
    } else if (parsed.kind === "assignVar") {
      const by = Object.fromEntries(state.map((b) => [b.name, b]));
      if (!by[parsed.name]) {
        const typeLabel = by[parsed.src]?.type || "int";
        return missingDeclError(parsed.name, typeLabel);
      }
      if (!by[parsed.src]) {
        return {
          error: `You can't use ${parsed.src} before declaring it.`,
          kind: "compile",
        };
      }
      if (requireSourceValue && String(by[parsed.src].value ?? "") === "") {
        return {
          error: `${parsed.src} doesn't have a value yet.`,
          kind: "ub",
        };
      }
    } else if (parsed.kind === "assignUnary") {
      const by = Object.fromEntries(state.map((b) => [b.name, b]));
      const base = by[parsed.name];
      const minDepth = minBaseDepthForOps(parsed.ops || []);
      const { base: baseType } = parseType(base?.type);
      const requiredBaseType =
        makePointerType(minDepth, baseType || "int") ||
        `int${"*".repeat(minDepth)}`;
      if (!base) {
        return missingDeclError(parsed.name, requiredBaseType);
      }
      const baseDepth = pointerDepth(base.type);
      if (
        baseDepth == null ||
        !Number.isFinite(baseDepth) ||
        baseDepth < minDepth
      ) {
        return typeMismatchError(parsed.name, requiredBaseType);
      }
      const resolved = resolveUnaryLvalue(state, parsed.ops, parsed.name);
      if (resolved?.error === "empty") {
        return {
          error: `${resolved.label} doesn't have a value yet, so it can't be dereferenced.`,
          kind: "ub",
        };
      }
      if (resolved?.error === "unknown") {
        return {
          error: `${resolved.label} doesn't point to a known variable.`,
          kind: "ub",
        };
      }
      if (resolved?.error === "not_deref") {
        return {
          error: `${resolved.label} is not a valid dereference.`,
          kind: "compile",
        };
      }
      if (resolved?.error === "not_lvalue") {
        return {
          error: "That assignment is not valid here.",
          kind: "compile",
        };
      }
      const target = resolved?.target;
      if (!target) {
        return {
          error: "That assignment is not valid here.",
          kind: "compile",
        };
      }
      if (parsed.rhs.kind === "num") {
        const err = numericLiteralErrorForType(parsed.rhs.value, target.type);
        if (err) return err;
      } else if (parsed.rhs.kind === "var") {
        const source = by[parsed.rhs.name];
        if (!source) {
          return {
            error: `You can't use ${parsed.rhs.name} before declaring it.`,
            kind: "compile",
          };
        }
        if (requireSourceValue && String(source.value ?? "") === "") {
          return {
            error: `${parsed.rhs.name} doesn't have a value yet.`,
            kind: "ub",
          };
        }
        const { base: targetBase, depth: targetDepth } = parseType(target.type);
        const { base: sourceBase, depth: sourceDepth } = parseType(source.type);
        const sameType = target.type === source.type;
        const isScalar =
          targetBase &&
          sourceBase &&
          targetBase === sourceBase &&
          targetDepth === 0 &&
          sourceDepth === 0;
        const isPtr = sameType && isPointerType(target.type);
        if (!isScalar && !isPtr) {
          return typeMismatchError(target.name, source.type);
        }
      } else if (parsed.rhs.kind === "ref") {
        const refBox = by[parsed.rhs.name];
        if (!refBox) {
          return {
            error: `You can't use ${parsed.rhs.name} before declaring it.`,
            kind: "compile",
          };
        }
        if (!isPointerType(target.type)) {
          const expected = expectedPointerTypeForRef(refBox.type) || "int*";
          return typeMismatchError(target.name, expected);
        }
        if (!isRefCompatible(target.type, refBox.type)) {
          const expected = expectedPointerTypeForRef(refBox.type);
          if (expected) return typeMismatchError(target.name, expected);
          return {
            error: "That assignment is not valid here.",
            kind: "compile",
          };
        }
      } else if (parsed.rhs.kind === "deref") {
        const ptr = by[parsed.rhs.name];
        if (!ptr) {
          return {
            error: `You can't use ${parsed.rhs.name} before declaring it.`,
            kind: "compile",
          };
        }
        const depth = parsed.rhs.depth || 1;
        const { base: ptrBase, depth: ptrDepth } = parseType(ptr.type);
        const { base: targetBase, depth: targetDepth } = parseType(target.type);
        if (!ptrBase || !targetBase) {
          return typeMismatchError(
            parsed.rhs.name,
            makePointerType(depth) || `int${"*".repeat(depth)}`,
          );
        }
        if (!Number.isFinite(ptrDepth) || ptrDepth < depth) {
          return typeMismatchError(
            parsed.rhs.name,
            makePointerType(depth, ptrBase) || `int${"*".repeat(depth)}`,
          );
        }
        const resultDepth = ptrDepth - depth;
        if (targetBase !== ptrBase || targetDepth !== resultDepth) {
          return typeMismatchError(
            target.name,
            makePointerType(resultDepth, ptrBase) ||
              `int${"*".repeat(resultDepth)}`,
          );
        }
        let current = ptr;
        const derefLabel = `${"*".repeat(depth)}${parsed.rhs.name}`;
        for (let i = 0; i < depth; i++) {
          if (!isPointerType(current.type)) {
            return typeMismatchError(
              parsed.rhs.name,
              makePointerType(depth, ptrBase) || `int${"*".repeat(depth)}`,
            );
          }
          if (String(current.value ?? "") === "") {
            return {
              error: `${derefLabel} doesn't have a value yet.`,
              kind: "ub",
            };
          }
          const next = state.find((b) => b.address === String(current.value));
          if (!next) {
            return {
              error: `${parsed.rhs.name} doesn't point to a known variable.`,
              kind: "ub",
            };
          }
          current = next;
        }
        if (requireSourceValue && String(current.value ?? "") === "") {
          return {
            error: `${derefLabel} doesn't have a value yet.`,
            kind: "ub",
          };
        }
      } else if (parsed.rhs.kind === "unary") {
        const err = validateUnaryRhs(
          state,
          target.type,
          target.name,
          parsed.rhs.ops,
          parsed.rhs.name,
        );
        if (err) return err;
      } else if (parsed.rhs.kind === "expr") {
        const { depth } = parseType(target.type);
        if (Number.isFinite(depth) && depth > 0) {
          return {
            error: "Pointer arithmetic is not supported here.",
            kind: "compile",
          };
        }
        const evaluated = evaluateExpression(parsed.rhs.expr, state, {
          allowVars: allowVarAssign,
          targetType: target.type,
        });
        if (evaluated.error) return evaluated;
      }
    } else if (parsed.kind === "assignUnaryRhs") {
      const by = Object.fromEntries(state.map((b) => [b.name, b]));
      const target = by[parsed.name];
      if (!target) {
        const typeLabel = "int";
        return missingDeclError(parsed.name, typeLabel);
      }
      const err = validateUnaryRhs(
        state,
        target.type,
        target.name,
        parsed.ops,
        parsed.src,
      );
      if (err) return err;
    } else if (parsed.kind === "assignRef") {
      const by = Object.fromEntries(state.map((b) => [b.name, b]));
      if (!by[parsed.name]) {
        const refType = by[parsed.ref]?.type || "int";
        const typeLabel = expectedPointerTypeForRef(refType) || "int*";
        return missingDeclError(parsed.name, typeLabel);
      }
      const refBox = by[parsed.ref];
      if (!refBox) {
        return {
          error: `You can't use ${parsed.ref} before declaring it.`,
          kind: "compile",
        };
      }
      if (!isPointerType(by[parsed.name].type)) {
        const expected = expectedPointerTypeForRef(refBox.type) || "int*";
        return typeMismatchError(parsed.name, expected);
      }
      if (!isRefCompatible(by[parsed.name].type, refBox.type)) {
        const expected = expectedPointerTypeForRef(refBox.type);
        if (expected) return typeMismatchError(parsed.name, expected);
        return {
          error: "That assignment is not valid here.",
          kind: "compile",
        };
      }
    } else if (
      parsed.kind === "assignDeref" ||
      parsed.kind === "assignDerefVar" ||
      parsed.kind === "assignDerefRef"
    ) {
      const by = Object.fromEntries(state.map((b) => [b.name, b]));
      const ptr = by[parsed.name];
      const depth = parsed.depth || 1;
      if (!ptr) {
        return missingDeclError(
          parsed.name,
          makePointerType(depth) || `int${"*".repeat(depth)}`,
        );
      }
      const { base: ptrBase, depth: ptrDepth } = parseType(ptr.type);
      if (!ptrBase || !Number.isFinite(ptrDepth) || ptrDepth < depth) {
        return typeMismatchError(
          parsed.name,
          makePointerType(depth, ptrBase || "int") || `int${"*".repeat(depth)}`,
        );
      }
      let expectedPtrDepth = depth;
      if (parsed.kind === "assignDerefVar") {
        if (!by[parsed.src]) {
          return {
            error: `You can't use ${parsed.src} before declaring it.`,
            kind: "compile",
          };
        }
        const { base: srcBase, depth: srcDepth } = parseType(
          by[parsed.src].type,
        );
        if (!srcBase || srcBase !== ptrBase) {
          return typeMismatchError(
            parsed.src,
            makePointerType(ptrDepth - depth, ptrBase) ||
              `int${"*".repeat(ptrDepth - depth)}`,
          );
        }
        expectedPtrDepth = Number.isFinite(srcDepth) ? depth + srcDepth : depth;
      }
      if (parsed.kind === "assignDerefRef") {
        if (!by[parsed.ref]) {
          return {
            error: `You can't use ${parsed.ref} before declaring it.`,
            kind: "compile",
          };
        }
        const { base: refBase, depth: refDepth } = parseType(
          by[parsed.ref].type,
        );
        if (!refBase || refBase !== ptrBase) {
          return typeMismatchError(
            parsed.ref,
            makePointerType(ptrDepth - depth, ptrBase) ||
              `int${"*".repeat(ptrDepth - depth)}`,
          );
        }
        expectedPtrDepth = Number.isFinite(refDepth)
          ? depth + refDepth + 1
          : depth + 1;
      }
      if (ptrDepth !== expectedPtrDepth) {
        return typeMismatchError(
          parsed.name,
          makePointerType(expectedPtrDepth, ptrBase) ||
            `int${"*".repeat(expectedPtrDepth)}`,
        );
      }
      let current = ptr;
      for (let i = 0; i < depth; i++) {
        if (!isPointerType(current.type)) {
          return typeMismatchError(
            parsed.name,
            makePointerType(depth, ptrBase) || `int${"*".repeat(depth)}`,
          );
        }
        if (String(current.value ?? "") === "") {
          return {
            error: `${parsed.name} doesn't have a value yet.`,
            kind: "ub",
          };
        }
        const next = state.find((b) => b.address === String(current.value));
        if (!next) {
          return {
            error: `${parsed.name} doesn't point to a known variable.`,
            kind: "ub",
          };
        }
        current = next;
      }
      if (parsed.kind === "assignDerefVar") {
        if (requireSourceValue && String(by[parsed.src].value ?? "") === "") {
          return {
            error: `${parsed.src} doesn't have a value yet.`,
            kind: "ub",
          };
        }
      }
      if (parsed.kind === "assignDeref") {
        const { target } = resolveDerefTarget(state, parsed.name, depth);
        if (target) {
          const err = numericLiteralErrorForType(parsed.value, target.type);
          if (err) return err;
        }
      }
    } else if (parsed.kind === "assignFromDeref") {
      const by = Object.fromEntries(state.map((b) => [b.name, b]));
      const target = by[parsed.name];
      if (!target) {
        return missingDeclError(parsed.name, "int");
      }
      const ptr = by[parsed.ptr];
      if (!ptr) {
        return {
          error: `You can't use ${parsed.ptr} before declaring it.`,
          kind: "compile",
        };
      }
      const depth = parsed.depth || 1;
      const { base: ptrBase, depth: ptrDepth } = parseType(ptr.type);
      const { base: targetBase, depth: targetDepth } = parseType(target.type);
      if (!ptrBase || !targetBase) {
        return typeMismatchError(
          parsed.ptr,
          makePointerType(depth) || `int${"*".repeat(depth)}`,
        );
      }
      if (!Number.isFinite(ptrDepth) || ptrDepth < depth) {
        return typeMismatchError(
          parsed.ptr,
          makePointerType(depth, ptrBase) || `int${"*".repeat(depth)}`,
        );
      }
      const resultDepth = ptrDepth - depth;
      if (targetBase !== ptrBase || targetDepth !== resultDepth) {
        return typeMismatchError(
          parsed.name,
          makePointerType(resultDepth, ptrBase) ||
            `int${"*".repeat(resultDepth)}`,
        );
      }
      let current = ptr;
      const derefLabel = `${"*".repeat(depth)}${parsed.ptr}`;
      for (let i = 0; i < depth; i++) {
        if (!isPointerType(current.type)) {
          return typeMismatchError(
            parsed.ptr,
            makePointerType(depth, ptrBase) || `int${"*".repeat(depth)}`,
          );
        }
        if (String(current.value ?? "") === "") {
          return {
            error: `${derefLabel} doesn't have a value yet.`,
            kind: "ub",
          };
        }
        const next = state.find((b) => b.address === String(current.value));
        if (!next) {
          return {
            error: `${parsed.ptr} doesn't point to a known variable.`,
            kind: "ub",
          };
        }
        current = next;
      }
      if (requireSourceValue && String(current.value ?? "") === "") {
        return {
          error: `${derefLabel} doesn't have a value yet.`,
          kind: "ub",
        };
      }
    }
    const next = applyStatement(state, parsed, {
      alloc,
      allowRedeclare: false,
    });
    if (!next)
      return { error: "That assignment is not valid here.", kind: "compile" };
    return { next, parsed };
  }

  function splitStatements(tokens: Token[]): StatementPart[] {
    const parts: StatementPart[] = [];
    let current: Token[] = [];
    let startLine = 0;
    for (const tok of tokens) {
      if (tok.type === "sym" && tok.value === ";") {
        parts.push({
          tokens: current,
          startLine: current[0]?.line ?? startLine,
          endLine: tok.line,
          hasSemicolon: true,
        });
        current = [];
        startLine = tok.line;
        continue;
      }
      if (isBraceToken(tok)) {
        if (current.length) {
          parts.push({
            tokens: current,
            startLine: current[0]?.line ?? startLine,
            endLine: current[current.length - 1].line,
            hasSemicolon: false,
          });
          current = [];
        }
        parts.push({
          tokens: [tok],
          startLine: tok.line,
          endLine: tok.line,
          hasSemicolon: true,
        });
        startLine = tok.line;
        continue;
      }
      if (!current.length) startLine = tok.line;
      current.push(tok);
    }
    if (current.length) {
      parts.push({
        tokens: current,
        startLine: current[0]?.line ?? startLine,
        endLine: current[current.length - 1].line,
        hasSemicolon: false,
      });
    }
    return parts;
  }

  function isBracePart(part: StatementPart, brace?: "{" | "}") {
    if (!part?.tokens?.length || part.tokens.length !== 1) return false;
    const tok = part.tokens[0];
    if (tok.type !== "sym") return false;
    if (brace) return tok.value === brace;
    return tok.value === "{" || tok.value === "}";
  }

  function buildIfStatementMap(
    parts: StatementPart[],
    opts: { lastLine?: number } = {},
  ): IfBlockMap {
    const map = new Map<number, IfBlock>();
    const errors = new Map<number, string>();
    const incomplete = new Set<number>();
    const fallbackLastLine =
      parts.length > 0
        ? Number.isFinite(parts[parts.length - 1]?.endLine)
          ? parts[parts.length - 1]!.endLine
          : 0
        : 0;
    const lastLine = Number.isFinite(opts.lastLine)
      ? Math.max(0, Number(opts.lastLine))
      : Math.max(0, fallbackLastLine);
    for (let i = 0; i < parts.length; i++) {
      const part = parts[i];
      if (!part?.tokens?.length) continue;
      const ifParsed = parseIfHeaderTokens(part.tokens);
      if (!ifParsed) continue;
      const headerStartLine = part.startLine;
      const headerEndLine = part.endLine;
      const openIndex = i + 1;
      const openPart = parts[openIndex];
      if (!openPart || !isBracePart(openPart, "{")) {
        errors.set(headerEndLine, "If statements must use braces.");
        continue;
      }
      let depth = 0;
      let closeIndex: number | null = null;
      for (let j = openIndex; j < parts.length; j++) {
        const probe = parts[j];
        if (isBracePart(probe, "{")) {
          depth++;
          continue;
        }
        if (isBracePart(probe, "}")) {
          depth--;
          if (depth === 0) {
            closeIndex = j;
            break;
          }
        }
      }
      if (closeIndex == null) {
        incomplete.add(lastLine);
        continue;
      }
      let trueTarget = openIndex;
      if (closeIndex > openIndex + 1) {
        trueTarget = openIndex + 1;
      }
      const falseTarget =
        closeIndex + 1 < parts.length ? closeIndex + 1 : parts.length;
      map.set(i, {
        headerIndex: i,
        headerStartLine,
        headerEndLine,
        openIndex,
        closeIndex,
        trueTarget,
        falseTarget,
        expr: ifParsed.expr,
        hasVar: ifParsed.hasVar,
      });
    }
    return { map, errors, incomplete };
  }

  function buildStatementMap(lines: string[]): StatementMap {
    const text = lines.join("\n");
    const tokens = tokenizeProgram(text);
    const parts = splitStatements(tokens);
    const byLine: Array<StatementRange | null> = new Array(lines.length).fill(
      null,
    );
    parts.forEach((part) => {
      if (!Number.isFinite(part.startLine) || !Number.isFinite(part.endLine))
        return;
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
    statementMap: StatementMap | null,
    lineIndex: number,
  ): StatementRange | null {
    if (!statementMap || !Array.isArray(statementMap.byLine)) return null;
    if (!Number.isFinite(lineIndex)) return null;
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
    const currentStart = currentRange?.startLine;
    const currentEnd = currentRange?.endLine;
    const isMultiLine =
      typeof currentStart === "number" &&
      typeof currentEnd === "number" &&
      Number.isFinite(currentStart) &&
      Number.isFinite(currentEnd) &&
      currentEnd > currentStart;
    const midStatement =
      isMultiLine && boundary > currentStart && boundary <= currentEnd;
    const atStatementStart = isMultiLine && boundary === currentStart;
    return {
      statementMap,
      currentRange,
      prevRange,
      midStatement,
      atStatementStart,
    };
  }

  function findMissingSemicolonLines(text: string): number[] {
    const lines = String(text ?? "").split(/\r?\n/);
    const missing: number[] = [];
    const patched: string[] = [];
    let inBlock = false;
    lines.forEach((line, idx) => {
      const raw = String(line ?? "");
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

  function classifyLineStatuses(
    lines: string[],
    opts: { alloc?: (type?: string) => string } = {},
  ): LineStatus {
    const invalid = new Set<number>();
    const incomplete = new Set<number>();
    const errors = new Map<number, any>();
    const errorKinds = new Map<number, string>();
    const info = new Map<number, any>();
    const text = lines.join("\n");
    const tokens = tokenizeProgram(text);
    let tokenIndex = 0;
    let currentTokens: Token[] = [];
    let state: BoxState[] = [];
    const alloc =
      opts.alloc || ((type?: string) => String(randAddr(type || "int")));
    const declared = new Set<string>();
    const scopes: ScopeStack = [new Set<string>()];
    const escapeHtml = (value: string) =>
      String(value)
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;")
        .replace(/'/g, "&#39;");
    const toStatementSnippet = (
      startLine: number,
      startCol: number,
      endLine: number,
    ) => {
      const safeStart = Math.max(0, startLine || 0);
      const safeEnd = Math.max(safeStart, endLine || 0);
      const parts: string[] = [];
      if (safeStart === safeEnd) {
        parts.push((lines[safeStart] || "").slice(startCol || 0));
      } else {
        parts.push((lines[safeStart] || "").slice(startCol || 0));
        for (let i = safeStart + 1; i <= safeEnd; i++) {
          parts.push(lines[i] || "");
        }
      }
      const joined = parts.join("\n");
      const stripped = stripAllComments(joined);
      return stripped.replace(/\n/g, " ").trim();
    };
    for (let lineIndex = 0; lineIndex < lines.length; lineIndex++) {
      let status = "";
      const applyStatementTokens = (
        tokensToValidate: Token[],
        endTok: Token,
        commit: boolean = true,
      ) => {
        if (!tokensToValidate.length) return;
        const result = validateStatement(
          tokensToValidate,
          state,
          declared,
          alloc,
        );
        if ("error" in result) {
          status = "invalid";
          errors.set(endTok.line, result.error);
          errorKinds.set(endTok.line, result.kind || "compile");
          const startLine = tokensToValidate[0]?.line;
          const startCol = tokensToValidate[0]?.col;
          const errKind = result.kind || "compile";
          if (
            errKind === "compile" &&
            Number.isFinite(startLine) &&
            endTok.line > startLine
          ) {
            const snippet = toStatementSnippet(startLine, startCol, endTok.line);
            const text = `This statement spans multiple lines and has a compilation error. In C, a line break acts like a space, so your statement is ${snippet}.`;
            const html = `This statement spans multiple lines and has a compilation error. In C, a line break acts like a space, so your statement is <code class="tok-code">${escapeHtml(snippet)}</code>.`;
            info.set(endTok.line, { text, html });
          }
          return;
        }
        if (commit) {
          const startLine = tokensToValidate[0]?.line;
          const startCol = tokensToValidate[0]?.col;
          if (Number.isFinite(startLine) && endTok.line > startLine) {
            const snippet = toStatementSnippet(
              startLine,
              startCol,
              endTok.line,
            );
            const text = `This statement spans multiple lines. In C, a line break acts like a space, so this statement is ${snippet}.`;
            const html = `This statement spans multiple lines. In C, a line break acts like a space, so this statement is <code class="tok-code">${escapeHtml(snippet)}</code>.`;
            info.set(endTok.line, { text, html });
          }
        }
        if (result.parsed.kind === "blockStart") {
          scopes.push(new Set<string>());
          return;
        }
        if (result.parsed.kind === "blockEnd") {
          const popped = popScope(scopes, declared, result.next);
          if (popped.error) {
            status = "invalid";
            errors.set(endTok.line, popped.error);
            errorKinds.set(endTok.line, "compile");
            return;
          }
          state = popped.state;
          return;
        }
        if (!commit) return;
        if (
          result.parsed.kind === "decl" ||
          result.parsed.kind === "declAssign" ||
          result.parsed.kind === "declAssignVar" ||
          result.parsed.kind === "declAssignRef" ||
          result.parsed.kind === "declAssignDeref" ||
          result.parsed.kind === "declAssignUnary"
        ) {
          addDeclaredName(scopes, declared, result.parsed.name);
        }
        state = result.next;
      };
      while (
        tokenIndex < tokens.length &&
        tokens[tokenIndex].line === lineIndex
      ) {
        const tok = tokens[tokenIndex];
        if (tok.type === "sym" && tok.value === ";") {
          if (currentTokens.length) {
            applyStatementTokens(currentTokens, tok);
            currentTokens = [];
          }
          tokenIndex++;
          continue;
        }
        if (isBraceToken(tok)) {
          if (currentTokens.length) {
            const endTok = currentTokens[currentTokens.length - 1];
            applyStatementTokens(currentTokens, endTok, false);
            currentTokens = [];
          }
          applyStatementTokens([tok], tok);
          tokenIndex++;
          continue;
        }
        currentTokens.push(tok);
        tokenIndex++;
      }
      if (status !== "invalid" && currentTokens.length) {
        const lastLine = currentTokens[currentTokens.length - 1]?.line;
        if (lastLine === lineIndex) {
          const lineText = lines[lineIndex] || "";
          const allowIntPrefix = !/\s$/.test(lineText);
          const isPrefix = isStatementPrefix(
            currentTokens,
            declared,
            allowIntPrefix,
          );
          if (!isPrefix) {
            status = "invalid";
            errors.set(lineIndex, describeTokensError(currentTokens, declared));
            errorKinds.set(lineIndex, "compile");
          }
        }
      }
      if (status === "invalid") invalid.add(lineIndex);
    }
    incomplete.clear();
    const parts = splitStatements(tokens);
    const ifBlocks = buildIfStatementMap(parts, {
      lastLine: Math.max(0, lines.length - 1),
    });
    parts.forEach((part, idx) => {
      if (!part?.tokens?.length) return;
      if (part.hasSemicolon) return;
      if (ifBlocks.map.has(idx)) return;
      if (
        part.tokens[0]?.type === "kw" &&
        part.tokens[0]?.value === "else"
      )
        return;
      if (!Number.isFinite(part.endLine)) return;
      incomplete.add(part.endLine);
      const start = Number.isFinite(part.startLine)
        ? part.startLine
        : part.endLine;
      for (let i = start; i <= part.endLine; i++) {
        if (invalid.has(i)) invalid.delete(i);
        if (errors.has(i)) errors.delete(i);
        if (errorKinds.has(i)) errorKinds.delete(i);
        if (info.has(i)) info.delete(i);
      }
    });
    incomplete.forEach((idx) => {
      if (invalid.has(idx)) invalid.delete(idx);
      if (errors.has(idx)) errors.delete(idx);
      if (errorKinds.has(idx)) errorKinds.delete(idx);
    });
    ifBlocks.errors.forEach((message, line) => {
      invalid.add(line);
      errors.set(line, message);
      errorKinds.set(line, "compile");
    });
    ifBlocks.incomplete.forEach((line) => {
      incomplete.add(line);
      if (invalid.has(line)) invalid.delete(line);
      if (errors.has(line)) errors.delete(line);
      if (errorKinds.has(line)) errorKinds.delete(line);
      if (info.has(line)) info.delete(line);
    });
    return { invalid, incomplete, errors, errorKinds, info };
  }

  function applyProgram(
    text: string,
    opts: { alloc?: (type?: string) => string } = {},
  ): BoxState[] | null {
    const tokens = tokenizeProgram(text);
    const parts = splitStatements(tokens);
    return applyProgramParts(parts, opts);
  }

  function applyProgramParts(
    parts: StatementPart[],
    opts: { alloc?: (type?: string) => string; stop?: number } = {},
  ): BoxState[] | null {
    let state: BoxState[] = [];
    const alloc =
      opts.alloc || ((type?: string) => String(randAddr(type || "int")));
    const stop =
      Number.isFinite(opts.stop) && opts.stop !== undefined
        ? Math.max(0, Math.min(parts.length, Number(opts.stop)))
        : null;
    const declared = new Set<string>();
    const scopes: ScopeStack = [new Set<string>()];
    const ifBlocks = buildIfStatementMap(parts);
    let i = 0;
    while (i < parts.length) {
      if (stop !== null && i >= stop) break;
      const part = parts[i];
      if (!part.tokens.length) {
        i += 1;
        continue;
      }
      const parsed = parseStatementTokens(part.tokens);
      if (!parsed) return null;
      if (parsed.kind === "if") {
        const block = ifBlocks.map.get(i);
        if (!block) return null;
        const result = evaluateCondition(parsed.expr, state);
        if ("error" in result) return null;
        if (result.value) {
          i += 1;
          continue;
        }
        if (stop !== null && stop <= block.closeIndex) break;
        i = block.closeIndex + 1;
        continue;
      }
      if (parsed.kind === "blockStart" || parsed.kind === "blockEnd") {
        if (parsed.kind === "blockStart") {
          scopes.push(new Set<string>());
          i += 1;
          continue;
        }
        const popped = popScope(scopes, declared, state);
        if (popped.error) return null;
        state = popped.state;
        i += 1;
        continue;
      }
      if (!part.hasSemicolon) return null;
      if (
        parsed.kind === "decl" ||
        parsed.kind === "declAssign" ||
        parsed.kind === "declAssignVar" ||
        parsed.kind === "declAssignRef" ||
        parsed.kind === "declAssignDeref" ||
        parsed.kind === "declAssignUnary"
      ) {
        if (declared.has(parsed.name)) return null;
      }
      const next = applyStatement(state, parsed, {
        alloc,
        allowRedeclare: false,
      });
      if (!next) return null;
      if (
        parsed.kind === "decl" ||
        parsed.kind === "declAssign" ||
        parsed.kind === "declAssignVar" ||
        parsed.kind === "declAssignRef" ||
        parsed.kind === "declAssignDeref" ||
        parsed.kind === "declAssignUnary"
      ) {
        addDeclaredName(scopes, declared, parsed.name);
      }
      state = next;
      i += 1;
    }
    return state;
  }

  function analyzeProgramParts(
    parts: StatementPart[],
    opts: { alloc?: (type?: string) => string; stop?: number } = {},
  ): ProgramResult {
    let state: BoxState[] = [];
    const alloc =
      opts.alloc || ((type?: string) => String(randAddr(type || "int")));
    const stop =
      Number.isFinite(opts.stop) && opts.stop !== undefined
        ? Math.max(0, Math.min(parts.length, Number(opts.stop)))
        : null;
    const declared = new Set<string>();
    const scopes: ScopeStack = [new Set<string>()];
    const ifBlocks = buildIfStatementMap(parts);
    let i = 0;
    while (i < parts.length) {
      if (stop !== null && i >= stop) break;
      const part = parts[i];
      if (!part.tokens.length) {
        i += 1;
        continue;
      }
      const result = validateStatement(part.tokens, state, declared, alloc);
      if ("error" in result) {
        return { kind: result.kind || "compile" };
      }
      const parsed = result.parsed;
      if (parsed.kind === "if") {
        const block = ifBlocks.map.get(i);
        if (!block) return { kind: "compile" };
        const condition = evaluateCondition(parsed.expr, state);
        if ("error" in condition) {
          return { kind: condition.kind || "compile" };
        }
        if (condition.value) {
          i += 1;
          continue;
        }
        if (stop !== null && stop <= block.closeIndex) break;
        i = block.closeIndex + 1;
        continue;
      }
      if (parsed.kind === "blockStart") {
        scopes.push(new Set<string>());
        i += 1;
        continue;
      }
      if (parsed.kind === "blockEnd") {
        const popped = popScope(scopes, declared, result.next);
        if (popped.error) return { kind: "compile" };
        state = popped.state;
        i += 1;
        continue;
      }
      if (!part.hasSemicolon) return { kind: "compile" };
      if (
        parsed.kind === "decl" ||
        parsed.kind === "declAssign" ||
        parsed.kind === "declAssignVar" ||
        parsed.kind === "declAssignRef" ||
        parsed.kind === "declAssignDeref" ||
        parsed.kind === "declAssignUnary"
      ) {
        addDeclaredName(scopes, declared, parsed.name);
      }
      state = result.next;
      i += 1;
    }
    return { kind: "ok", state };
  }

  return {
    tokenizeProgram,
    splitStatements,
    parseStatements,
    buildStatementMap,
    buildIfStatementMap,
    statementRangeForLine,
    getStatementContext,
    evaluateCondition,
    evaluateExpressionText,
    classifyLineStatuses,
    findMissingSemicolonLines,
    applyProgramParts,
    analyzeProgramParts,
    applyProgram,
  };
}
