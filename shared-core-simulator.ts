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
export type Statement =
  | { kind: "blockStart" }
  | { kind: "blockEnd" }
  | { kind: "if"; expr: ExprNode; hasVar: boolean }
  | { kind: "else" }
  | { kind: "decl"; name: string; type: string }
  | { kind: "assign"; lhs: ExprNode; rhs: ExprNode; hasVar: boolean }
  | {
      kind: "declAssign";
      name: string;
      declType?: string;
      expr: ExprNode;
      hasVar: boolean;
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
  elseIndex?: number | null;
  elseOpenIndex?: number | null;
  elseCloseIndex?: number | null;
  elseTarget?: number | null;
  afterIndex: number;
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
  } = {},
): SimpleSimulator {
  const { allowVarAssign = false, requireSourceValue = false } = opts;

  type DeclaredNames = Set<string>;
  type ScopeStack = Array<Set<string>>;
  type ExprParseResult = {
    expr: ExprNode;
    nextIndex: number;
    hasVar: boolean;
  };
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
        let isDecimal = false;
        if (src[j] === "0" && (src[j + 1] === "x" || src[j + 1] === "X")) {
          j += 2;
          while (j < src.length && /[0-9a-fA-F]/.test(src[j])) j++;
        } else {
          while (j < src.length && /[0-9]/.test(src[j])) j++;
          if (src[j] === ".") {
            isDecimal = true;
            j++;
            while (j < src.length && /[0-9]/.test(src[j])) j++;
          }
          if (src[j] === "e" || src[j] === "E") {
            isDecimal = true;
            let k = j + 1;
            if (src[k] === "+" || src[k] === "-") k++;
            const expStart = k;
            while (k < src.length && /[0-9]/.test(src[k])) k++;
            if (k > expStart) {
              j = k;
            }
          }
        }
        if (!isDecimal && (src[j] === "l" || src[j] === "L")) {
          j++;
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
    return `${baseType}${"*".repeat(stars)}`;
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

  function stripIntegerSuffix(value: string): string {
    const raw = value.trim();
    if (!raw) return raw;
    const last = raw[raw.length - 1];
    if (last === "l" || last === "L") return raw.slice(0, -1);
    return raw;
  }

  function hasLongSuffix(value: string): boolean {
    const raw = value.trim();
    if (!raw) return false;
    const last = raw[raw.length - 1];
    return last === "l" || last === "L";
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
      if (base !== "double") {
        const truncated = Math.trunc(parsed);
        try {
          const asInt = BigInt(truncated);
          return checkIntegerRange(asInt, base) || null;
        } catch {
          return integerOverflowError(base);
        }
      }
      return null;
    }
    if (base === "double") return null;
    const status = classifyNumericLiteral(value);
    if (base === "long") {
      return status === "compile" ? numericLiteralError(status) : null;
    }
    if (base === "int") {
      if (status === "compile") return numericLiteralError(status);
      return null;
    }
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

  function wrapIntegerToBase(value: bigint, base: string): bigint | null {
    const width = bitWidthForBase(base);
    if (!width) return null;
    const modulo = 1n << BigInt(width);
    let wrapped = value % modulo;
    if (wrapped < 0n) wrapped += modulo;
    const signBit = 1n << BigInt(width - 1);
    if (wrapped >= signBit) wrapped -= modulo;
    return wrapped;
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
          if (tok.value === "*" || tok.value === "&") {
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

  function isAssignPrefix(tokens: Token[]): boolean {
    if (!tokens.length) return false;
    const eqIndex = tokens.findIndex(
      (tok) => tok.type === "sym" && tok.value === "=",
    );
    if (eqIndex === -1) return false;
    if (eqIndex === 0) return false;
    const lhsTokens = tokens.slice(0, eqIndex);
    if (!isExpressionPrefix(lhsTokens, { allowVars: true })) return false;
    if (eqIndex === tokens.length - 1) return true;
    return isExpressionPrefix(tokens.slice(eqIndex + 1), {
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
    if (tokens[0].type === "kw" && tokens[0].value === "else") {
      return tokens.length === 1;
    }
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
    if (t0.type === "sym" && t0.value === "*") return true;
    }
    return (
      isIfPrefix(tokens) ||
      isDeclPrefix(tokens) ||
      isAssignPrefix(tokens)
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
          (tok.value === "*" || tok.value === "&"))
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
          const literalBase = hasLongSuffix(node.value)
            ? "long"
            : literalStatus === "ub"
              ? "long"
              : "int";
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
            error: "Assignments should not use variables yet.",
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
          const target = state.find((b) => (b.address ?? "") === ptrVal);
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
    if (targetBase !== "int" && targetBase !== "long") return null;
    let numValue: number;
    if (base === "double") {
      const num = typeof value === "bigint" ? Number(value) : value;
      if (!Number.isFinite(num)) return integerOverflowError(targetBase);
      numValue = Math.trunc(num);
    } else if (typeof value === "bigint") {
      if (targetBase === "int") {
        const wrapped = wrapIntegerToBase(value, targetBase);
        return { value: wrapped ?? value, base: targetBase };
      }
      const overflow = checkIntegerRange(value, targetBase);
      if (overflow) return overflow;
      return { value, base: targetBase };
    } else {
      numValue = Math.trunc(value);
    }
    try {
      const asInt = BigInt(numValue);
      const overflow = checkIntegerRange(asInt, targetBase);
      if (overflow) return overflow;
      return { value: asInt, base: targetBase };
    } catch {
      return null;
    }
  }

  function convertAssignmentValue(
    evaluated: EvalResult,
    targetType: string,
    requireValue: boolean,
  ):
    | { value: string; nanSign?: -1 | 1 }
    | EvalError
    | { kind: "type-mismatch"; expectedType: string } {
    if (isEvalError(evaluated)) return evaluated;
    const { base: targetBase, depth: targetDepth } = parseType(targetType);
    if (!targetBase || !Number.isFinite(targetDepth)) {
      return { error: "That assignment is not valid here.", kind: "compile" };
    }
    if (targetDepth === 0) {
      const scalar = coerceScalarResult(evaluated, requireValue);
      if (isScalarError(scalar)) return scalar;
      const converted = convertScalarForAssignment(
        scalar.value,
        scalar.base,
        targetType,
        scalar.nanSign,
      );
      if (!converted) {
        return { error: "That assignment is not valid here.", kind: "compile" };
      }
      if ("error" in converted) return converted;
      return {
        value: formatValueForType(converted.value, targetType, {
          nanSign: converted.nanSign,
        }),
        nanSign: converted.nanSign,
      };
    }
    const evalDepth =
      Number.isFinite(evaluated.depth) && evaluated.depth !== undefined
        ? evaluated.depth
        : 0;
    const evalBase = evaluated.base || "int";
    if (evalDepth !== targetDepth || evalBase !== targetBase) {
      const expectedType =
        makePointerType(evalDepth, evalBase) || `int${"*".repeat(evalDepth)}`;
      return { kind: "type-mismatch", expectedType };
    }
    if (requireValue && String(evaluated.value ?? "") === "") {
      const label = evaluated.label || "That value";
      return { error: `${label} doesn't have a value yet.`, kind: "ub" };
    }
    return {
      value: formatValueForType(evaluated.value, targetType, {
        nanSign: evaluated.nanSign,
      }),
      nanSign: evaluated.nanSign,
    };
  }

  function validateAssignmentExpr(
    state: BoxState[],
    targetType: string,
    targetName: string,
    expr: ExprNode,
    { allowVars = allowVarAssign }: { allowVars?: boolean } = {},
  ): EvalError | null {
    const evaluated = evaluateExpressionRaw(expr, state, {
      allowVars,
      targetType,
    });
    const converted = convertAssignmentValue(
      evaluated,
      targetType,
      requireSourceValue,
    );
    if ("kind" in converted && converted.kind === "type-mismatch") {
      return typeMismatchError(targetName, converted.expectedType);
    }
    if ("error" in converted) return converted;
    return null;
  }

  function parseAssignRhs(
    tokens: Token[],
    idx: number,
    { allowVar }: { allowVar?: boolean } = {},
  ): ExprParseResult | null {
    if (idx >= tokens.length) return null;
    const allowVars = allowVar ?? allowVarAssign;
    const parsed = parseExpressionTokens(tokens, idx, { allowVars });
    if (!parsed || parsed.nextIndex !== tokens.length) return null;
    return { expr: parsed.expr, hasVar: parsed.hasVar, nextIndex: parsed.nextIndex };
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
    if (
      tokens.length === 1 &&
      tokens[0].type === "kw" &&
      tokens[0].value === "else"
    ) {
      return { kind: "else" };
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
      return {
        kind: "declAssign",
        name,
        declType,
        expr: rhs.expr,
        hasVar: rhs.hasVar,
      };
    }
    if (tokens.length >= 3) {
      const eqIndex = tokens.findIndex(
        (tok) => tok.type === "sym" && tok.value === "=",
      );
      if (eqIndex <= 0 || eqIndex >= tokens.length - 1) return null;
      const lhsTokens = tokens.slice(0, eqIndex);
      const rhs = parseAssignRhs(tokens, eqIndex + 1, {
        allowVar: allowVarAssign,
      });
      if (!rhs) return null;
      const lhsParsed = parseExpressionTokens(lhsTokens, 0, {
        allowVars: true,
      });
      if (!lhsParsed || lhsParsed.nextIndex !== lhsTokens.length) return null;
      return {
        kind: "assign",
        lhs: lhsParsed.expr,
        rhs: rhs.expr,
        hasVar: rhs.hasVar || lhsParsed.hasVar,
      };
    }
    return null;
  }

  function applyAssignmentToTarget(
    boxes: BoxState[],
    target: BoxState,
    targetType: string,
    expr: ExprNode,
    { allowVars = allowVarAssign }: { allowVars?: boolean } = {},
  ): BoxState[] | null {
    const evaluated = evaluateExpressionRaw(expr, boxes, {
      allowVars,
      targetType,
    });
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
    const evaluated = evaluateExpressionRaw(lhs, state, {
      allowVars: true,
      targetType: "int",
    });
    if (isEvalError(evaluated)) return evaluated;
    if (evaluated.kind !== "lvalue") {
      return { error: "That assignment is not valid here.", kind: "compile" };
    }
    const { base, depth } = evaluated;
    const targetType =
      makePointerType(
        Number.isFinite(depth) ? depth : 0,
        base || "int",
      ) || "int";
    const target = state.find(
      (b) => (b.address ?? "") === (evaluated.address ?? ""),
    );
    if (!target) {
      return { error: "That assignment is not valid here.", kind: "compile" };
    }
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
    if (stmt.kind === "assign") {
      const resolved = resolveAssignmentTarget(boxes, stmt.lhs);
      if ("error" in resolved) return null;
      return applyAssignmentToTarget(
        boxes,
        resolved.target,
        resolved.target.type,
        stmt.rhs,
        { allowVars: allowVarAssign },
      );
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
      const target = by[stmt.name] || boxes.find((b) => b.name === stmt.name);
      if (!target) return null;
      return applyAssignmentToTarget(boxes, target, declType, stmt.expr, {
        allowVars: allowVarAssign,
      });
    }
    return null;
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
      return 'Else statements should look like "else { ... }".';
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
    if (tokens[0].type === "ident") {
      const name = tokens[0].value;
      if (tokens[1]?.type === "ident") {
        return `${name} isn't a valid type name.`;
      }
      if (!hasDeclaredPrefix(name, seenDecl))
        return `You can't use ${name} before declaring it.`;
      if (tokens.length === 1)
        return 'Assignments should look like "expression = expression;".';
      if (tokens[1].type !== "sym" || tokens[1].value !== "=")
        return 'Assignments should use "=".';
      if (tokens.length === 2) return "Assignment needs a value on the right.";
      const rhs = tokens[2];
      if (rhs.type === "ident" && !allowVarAssign)
        return "Assignments should not use variables yet.";
      if (rhs.type === "ident" && !hasDeclaredPrefix(rhs.value, seenDecl)) {
        return `You can't use ${rhs.value} before declaring it.`;
      }
      return 'Assignments should look like "expression = expression;".';
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
    if (parsed.kind === "else") {
      return { parsed, next: state };
    }
    if (parsed.kind === "if") {
      const result = evaluateCondition(parsed.expr, state);
      if ("error" in result) {
        return { error: result.error, kind: result.kind };
      }
      return { parsed, next: state };
    }
    if (parsed.kind === "decl" || parsed.kind === "declAssign") {
      if (seenDecl.has(parsed.name))
        return {
          error: `You already declared ${parsed.name}.`,
          kind: "compile",
        };
      if (parsed.kind === "declAssign") {
        const targetType = parsed.declType || "int";
        const err = validateAssignmentExpr(
          state,
          targetType,
          parsed.name,
          parsed.expr,
          { allowVars: allowVarAssign },
        );
        if (err) return err;
      }
    } else if (parsed.kind === "assign") {
      const resolved = resolveAssignmentTarget(state, parsed.lhs);
      if ("error" in resolved) return resolved;
      const err = validateAssignmentExpr(
        state,
        resolved.target.type,
        resolved.target.name,
        parsed.rhs,
        { allowVars: allowVarAssign },
      );
      if (err) return err;
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

  function isElsePart(part: StatementPart | null): boolean {
    if (!part?.tokens?.length || part.tokens.length !== 1) return false;
    const tok = part.tokens[0];
    return tok.type === "kw" && tok.value === "else";
  }

  function buildIfStatementMap(
    parts: StatementPart[],
    opts: { lastLine?: number } = {},
  ): IfBlockMap {
    const map = new Map<number, IfBlock>();
    const errors = new Map<number, string>();
    const incomplete = new Set<number>();
    const usedElse = new Set<number>();
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
      let elseIndex: number | null = null;
      let elseOpenIndex: number | null = null;
      let elseCloseIndex: number | null = null;
      let elseTarget: number | null = null;
      let afterIndex = closeIndex + 1;
      const possibleElseIndex = closeIndex + 1;
      const possibleElse = parts[possibleElseIndex];
      if (isElsePart(possibleElse)) {
        elseIndex = possibleElseIndex;
        usedElse.add(possibleElseIndex);
        const elseOpenPart = parts[possibleElseIndex + 1];
        if (!elseOpenPart || !isBracePart(elseOpenPart, "{")) {
          errors.set(
            possibleElse?.endLine ?? headerEndLine,
            "Else statements must use braces.",
          );
          continue;
        }
        elseOpenIndex = possibleElseIndex + 1;
        let elseDepth = 0;
        let foundElseClose: number | null = null;
        for (let j = elseOpenIndex; j < parts.length; j++) {
          const probe = parts[j];
          if (isBracePart(probe, "{")) {
            elseDepth++;
            continue;
          }
          if (isBracePart(probe, "}")) {
            elseDepth--;
            if (elseDepth === 0) {
              foundElseClose = j;
              break;
            }
          }
        }
        if (foundElseClose == null) {
          incomplete.add(lastLine);
          continue;
        }
        elseCloseIndex = foundElseClose;
        afterIndex = elseCloseIndex + 1;
        elseTarget = elseOpenIndex;
        if (elseCloseIndex > elseOpenIndex + 1) {
          elseTarget = elseOpenIndex + 1;
        }
      }
      const falseTarget =
        elseTarget ??
        (afterIndex < parts.length ? afterIndex : parts.length);
      map.set(i, {
        headerIndex: i,
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
    }
    parts.forEach((part, idx) => {
      if (!isElsePart(part)) return;
      if (usedElse.has(idx)) return;
      const line = Number.isFinite(part.endLine) ? part.endLine : lastLine;
      errors.set(line, "Else statements must follow an if statement.");
    });
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
          result.parsed.kind === "declAssign"
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
    const ifDecisions = new Map<number, boolean>();
    const elseLookup = new Map<number, IfBlock>();
    ifBlocks.map.forEach((block) => {
      if (block.elseIndex != null) {
        elseLookup.set(block.elseIndex, block);
      }
    });
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
        ifDecisions.set(block.headerIndex, result.value);
        if (result.value) {
          i += 1;
          continue;
        }
        if (stop !== null && stop <= block.closeIndex) break;
        if (block.elseOpenIndex != null) {
          i = block.elseOpenIndex;
          continue;
        }
        i = block.closeIndex + 1;
        continue;
      }
      if (parsed.kind === "else") {
        const block = elseLookup.get(i);
        if (!block) return null;
        const decision = ifDecisions.get(block.headerIndex);
        if (decision == null) return null;
        if (decision) {
          i = block.afterIndex;
          continue;
        }
        if (block.elseOpenIndex != null) {
          i = block.elseOpenIndex;
          continue;
        }
        return null;
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
      if (parsed.kind === "decl" || parsed.kind === "declAssign") {
        if (declared.has(parsed.name)) return null;
      }
      const next = applyStatement(state, parsed, {
        alloc,
        allowRedeclare: false,
      });
      if (!next) return null;
      if (parsed.kind === "decl" || parsed.kind === "declAssign") {
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
    const ifDecisions = new Map<number, boolean>();
    const elseLookup = new Map<number, IfBlock>();
    ifBlocks.map.forEach((block) => {
      if (block.elseIndex != null) {
        elseLookup.set(block.elseIndex, block);
      }
    });
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
        ifDecisions.set(block.headerIndex, condition.value);
        if (condition.value) {
          i += 1;
          continue;
        }
        if (stop !== null && stop <= block.closeIndex) break;
        if (block.elseOpenIndex != null) {
          i = block.elseOpenIndex;
          continue;
        }
        i = block.closeIndex + 1;
        continue;
      }
      if (parsed.kind === "else") {
        const block = elseLookup.get(i);
        if (!block) return { kind: "compile" };
        const decision = ifDecisions.get(block.headerIndex);
        if (decision == null) return { kind: "compile" };
        if (decision) {
          i = block.afterIndex;
          continue;
        }
        if (block.elseOpenIndex != null) {
          i = block.elseOpenIndex;
          continue;
        }
        return { kind: "compile" };
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
      if (parsed.kind === "decl" || parsed.kind === "declAssign") {
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
