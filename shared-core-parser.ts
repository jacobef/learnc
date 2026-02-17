import { canonicalizeBaseType, normalizeSpecialFloatLiteral } from "./shared-core-utils.js";

export type UnaryOp = "+" | "-" | "*" | "&" | "~" | "!" | "++" | "--";
export type PostfixOp = "++" | "--";
export type BinaryOp =
  | "||"
  | "&&"
  | "|"
  | "^"
  | "&"
  | "=="
  | "!="
  | ">"
  | ">="
  | "<"
  | "<="
  | "<<"
  | ">>"
  | "+"
  | "-"
  | "*"
  | "/"
  | "%";
export type AssignmentOp =
  | "="
  | "+="
  | "-="
  | "*="
  | "/="
  | "%="
  | "<<="
  | ">>="
  | "&="
  | "^="
  | "|=";
export type ExprNode =
  | { kind: "num"; value: string }
  | { kind: "var"; name: string }
  | { kind: "cast"; targetType: string; expr: ExprNode }
  | { kind: "unary"; op: UnaryOp; expr: ExprNode }
  | { kind: "postfix"; op: PostfixOp; expr: ExprNode }
  | { kind: "subscript"; left: ExprNode; index: ExprNode }
  | { kind: "binary"; op: BinaryOp; left: ExprNode; right: ExprNode }
  | { kind: "assign"; op: AssignmentOp; left: ExprNode; right: ExprNode };
export type Statement =
  | { kind: "blockStart" }
  | { kind: "blockEnd" }
  | { kind: "empty" }
  | { kind: "if"; expr: ExprNode; hasVar: boolean }
  | { kind: "while"; expr: ExprNode; hasVar: boolean }
  | { kind: "else" }
  | {
      kind: "decl";
      name: string;
      type: string;
      arrayShape?: number[];
      elementType?: string;
      pointeeArrayDims?: number[];
      pointeeInnerDepth?: number;
      elementPointeeArrayDims?: number[];
      elementPointeeInnerDepth?: number;
      declaredNames: string[];
    }
  | {
      kind: "assign";
      op: AssignmentOp;
      lhs: ExprNode;
      rhs: ExprNode;
      hasVar: boolean;
    }
  | { kind: "expr"; expr: ExprNode; hasVar: boolean }
  | {
      kind: "declAssign";
      name: string;
      declType?: string;
      pointeeArrayDims?: number[];
      pointeeInnerDepth?: number;
      expr: ExprNode;
      hasVar: boolean;
      declaredNames: string[];
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
export type DeclaredNames = Set<string>;
export type ExprParseResult = {
  expr: ExprNode;
  nextIndex: number;
  hasVar: boolean;
};
export type DeclHeadParseResult = {
  kind: "none" | "partial" | "full";
  declType?: string;
  pointeeArrayDims?: number[];
  pointeeInnerDepth?: number;
  elementType?: string;
  elementPointeeArrayDims?: number[];
  elementPointeeInnerDepth?: number;
  arrayShape?: number[];
  name?: string;
  rhsStart?: number;
  hasInitializer?: boolean;
};
export interface ParserTools {
  tokenizeProgram: (src?: string) => Token[];
  parseExpressionTokens: (
    tokens: Token[],
    start: number,
    opts?: { allowVars?: boolean },
  ) => ExprParseResult | null;
  parseDeclHead: (tokens: Token[]) => DeclHeadParseResult;
  parseIfHeaderTokens: (
    tokens: Token[],
  ) => { expr: ExprNode; hasVar: boolean } | null;
  parseWhileHeaderTokens: (
    tokens: Token[],
  ) => { expr: ExprNode; hasVar: boolean } | null;
  parseStatementTokens: (tokens: Token[]) => Statement | null;
  isStatementPrefix: (
    tokens: Token[],
    declaredNames: DeclaredNames,
    allowIntPrefix: boolean,
  ) => boolean;
  controlHeaderEndIndex: (tokens: Token[]) => number;
  splitStatements: (tokens: Token[]) => StatementPart[];
}
interface ParserOptions {
  evaluateArrayLengthExpr: (expr: ExprNode) => number | null;
}

const TYPE_KEYWORDS = new Set([
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

type DeclDerivedOp = { kind: "ptr" } | { kind: "array"; length: number };
type DeclTypeNode =
  | { kind: "base"; base: string }
  | { kind: "ptr"; to: DeclTypeNode }
  | { kind: "array"; length: number; of: DeclTypeNode };
type DeclTypeInfo = {
  base: string;
  depth: number;
  outerPointerDepth: number;
  innerPointerDepth: number;
  pointeeArrayDims: number[];
};

function hasDeclaredPrefix(prefix: string, names: DeclaredNames | null) {
  if (!prefix || !names || !names.size) return false;
  for (const name of names) {
    if (name.startsWith(prefix)) return true;
  }
  return false;
}

function arrayElementName(name: string, indices: number[]): string {
  return `${name}${indices.map((index) => `[${index}]`).join("")}`;
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

function makeArrayDeclaredNames(name: string, shape: number[]): string[] {
  const out = [name];
  forEachArrayIndex(shape, (indices) => {
    out.push(arrayElementName(name, indices));
  });
  return out;
}

function makeSimplePointerType(depth: number, base: string): string | null {
  const stars = Math.max(0, Math.floor(Number(depth)));
  const canonical = canonicalizeBaseType(base);
  if (!canonical) return null;
  return `${canonical}${"*".repeat(stars)}`;
}

function buildDeclType(base: string, opsOutward: DeclDerivedOp[]): DeclTypeNode {
  let node: DeclTypeNode = { kind: "base", base };
  for (let i = opsOutward.length - 1; i >= 0; i--) {
    const op = opsOutward[i]!;
    if (op.kind === "ptr") {
      node = { kind: "ptr", to: node };
    } else {
      node = { kind: "array", length: op.length, of: node };
    }
  }
  return node;
}

function peelArrayType(node: DeclTypeNode): { shape: number[]; element: DeclTypeNode } {
  const shape: number[] = [];
  let current = node;
  while (current.kind === "array") {
    shape.push(current.length);
    current = current.of;
  }
  return { shape, element: current };
}

function lowerDeclType(node: DeclTypeNode): DeclTypeInfo | null {
  let outerPointerDepth = 0;
  let current = node;
  while (current.kind === "ptr") {
    outerPointerDepth++;
    current = current.to;
  }
  let pointeeArrayDims: number[] = [];
  if (outerPointerDepth > 0 && current.kind === "array") {
    const peeled = peelArrayType(current);
    pointeeArrayDims = peeled.shape;
    current = peeled.element;
  }
  let innerPointerDepth = 0;
  while (current.kind === "ptr") {
    innerPointerDepth++;
    current = current.to;
  }
  if (current.kind !== "base") return null;
  return {
    base: current.base,
    depth: outerPointerDepth + innerPointerDepth,
    outerPointerDepth,
    innerPointerDepth,
    pointeeArrayDims,
  };
}

function formatDeclType(info: DeclTypeInfo): string | null {
  const base = canonicalizeBaseType(info.base);
  if (!base) return null;
  if (!info.pointeeArrayDims.length) {
    return `${base}${"*".repeat(Math.max(0, info.depth))}`;
  }
  const inner = info.innerPointerDepth > 0 ? ` ${"*".repeat(info.innerPointerDepth)}` : "";
  const outer = "*".repeat(Math.max(1, info.outerPointerDepth));
  const dims = info.pointeeArrayDims.map((d) => `[${d}]`).join("");
  return `${base}${inner} (${outer})${dims}`;
}

function isBraceToken(tok: Token): boolean {
  return tok.type === "sym" && (tok.value === "{" || tok.value === "}");
}

export function createParserTools(opts: ParserOptions): ParserTools {
  const { evaluateArrayLengthExpr } = opts;

  function tokenizeProgram(src = ""): Token[] {
    const tokens: Token[] = [];
    let i = 0;
    let line = 0;
    let col = 0;
    const startsWith = (value: string) => src.startsWith(value, i);
    const advanceBy = (count: number) => {
      for (let k = 0; k < count; k++) {
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
        i++;
        col++;
      }
    };
    const pushSym = (value: string) => {
      tokens.push({ type: "sym", value, line, col });
      advanceBy(value.length);
    };
    const parseCharLiteral = (): {
      value: string;
      startLine: number;
      startCol: number;
    } | null => {
      if (src[i] !== "'") return null;
      const startLine = line;
      const startCol = col;
      advanceBy(1);
      if (i >= src.length) {
        tokens.push({ type: "unknown", value: "'", line: startLine, col: startCol });
        return null;
      }
      let code: number | null = null;
      const ch = src[i];
      if (ch === "\\") {
        advanceBy(1);
        if (i >= src.length) {
          tokens.push({ type: "unknown", value: "'", line: startLine, col: startCol });
          return null;
        }
        const esc = src[i];
        const escapeMap: Record<string, number> = {
          a: 7,
          b: 8,
          f: 12,
          n: 10,
          r: 13,
          t: 9,
          v: 11,
          "\\": 92,
          "'": 39,
          '"': 34,
          "0": 0,
        };
        if (!(esc in escapeMap)) {
          tokens.push({ type: "unknown", value: "'", line: startLine, col: startCol });
          return null;
        }
        code = escapeMap[esc];
        advanceBy(1);
      } else {
        if (ch === "'" || ch === "\n" || ch === "\r") {
          tokens.push({ type: "unknown", value: "'", line: startLine, col: startCol });
          return null;
        }
        code = ch.charCodeAt(0);
        advanceBy(1);
      }
      if (src[i] !== "'") {
        tokens.push({ type: "unknown", value: "'", line: startLine, col: startCol });
        return null;
      }
      advanceBy(1);
      if (code == null) {
        tokens.push({ type: "unknown", value: "'", line: startLine, col: startCol });
        return null;
      }
      return { value: String(code), startLine, startCol };
    };
    const parseNumberLiteral = (): string | null => {
      if (!(/[0-9]/.test(src[i] || "") || (src[i] === "." && /[0-9]/.test(src[i + 1] || "")))) {
        return null;
      }
      let j = i;
      let sawDot = false;
      let sawExponent = false;
      if (src[j] === ".") {
        sawDot = true;
        j++;
      }
      if (src[j] === "0" && (src[j + 1] === "x" || src[j + 1] === "X")) {
        j += 2;
        const startHex = j;
        while (j < src.length && /[0-9a-fA-F]/.test(src[j])) j++;
        if (j === startHex) return null;
        while (j < src.length && /[uUlL]/.test(src[j])) j++;
        return src.slice(i, j);
      }
      while (j < src.length && /[0-9]/.test(src[j])) j++;
      if (src[j] === ".") {
        sawDot = true;
        j++;
        while (j < src.length && /[0-9]/.test(src[j])) j++;
      }
      if (src[j] === "e" || src[j] === "E") {
        const expMark = j;
        let k = j + 1;
        if (src[k] === "+" || src[k] === "-") k++;
        const expStart = k;
        while (k < src.length && /[0-9]/.test(src[k])) k++;
        if (k > expStart) {
          sawExponent = true;
          j = k;
        } else {
          j = expMark;
        }
      }
      if (sawDot || sawExponent) {
        if (src[j] === "f" || src[j] === "F" || src[j] === "l" || src[j] === "L") {
          j++;
        }
        return src.slice(i, j);
      }
      while (j < src.length && /[uUlL]/.test(src[j])) j++;
      return src.slice(i, j);
    };
    const keywordSet = new Set([
      "if",
      "while",
      "else",
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
    const multiCharSymbols = [
      "<<=",
      ">>=",
      "++",
      "--",
      "&&",
      "||",
      "+=",
      "-=",
      "*=",
      "/=",
      "%=",
      "&=",
      "|=",
      "^=",
      "==",
      "!=",
      "<=",
      ">=",
      "<<",
      ">>",
    ];
    const singleCharSymbols = new Set([
      "=",
      "!",
      "<",
      ">",
      "+",
      "-",
      "*",
      "/",
      "%",
      "&",
      "|",
      "^",
      "~",
      ";",
      ",",
      "(",
      ")",
      "[",
      "]",
      "{",
      "}",
    ]);
    while (i < src.length) {
      const ch = src[i];
      if (ch === "\r") {
        advanceBy(1);
        continue;
      }
      if (ch === "\n") {
        advanceBy(1);
        continue;
      }
      if (ch === "/" && src[i + 1] === "/") {
        while (i < src.length && src[i] !== "\n" && src[i] !== "\r") advanceBy(1);
        continue;
      }
      if (ch === "/" && src[i + 1] === "*") {
        const startLine = line;
        const startCol = col;
        advanceBy(2);
        let closed = false;
        while (i < src.length) {
          if (startsWith("*/")) {
            advanceBy(2);
            closed = true;
            break;
          }
          advanceBy(1);
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
        advanceBy(1);
        continue;
      }
      const charLiteral = parseCharLiteral();
      if (charLiteral != null) {
        tokens.push({
          type: "number",
          value: charLiteral.value,
          line: charLiteral.startLine,
          col: charLiteral.startCol,
        });
        continue;
      }
      const numberLiteral = parseNumberLiteral();
      if (numberLiteral != null) {
        const startCol = col;
        advanceBy(numberLiteral.length);
        tokens.push({ type: "number", value: numberLiteral, line, col: startCol });
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
            : keywordSet.has(ident)
              ? "kw"
              : "ident",
          value: special || ident,
          line,
          col: startCol,
        });
        advanceBy(j - i);
        continue;
      }
      let matchedSymbol = "";
      for (const sym of multiCharSymbols) {
        if (startsWith(sym)) {
          matchedSymbol = sym;
          break;
        }
      }
      if (matchedSymbol) {
        pushSym(matchedSymbol);
        continue;
      }
      if (singleCharSymbols.has(ch)) {
        pushSym(ch);
        continue;
      }
      tokens.push({ type: "unknown", value: ch, line, col });
      advanceBy(1);
    }
    return tokens;
  }

  function exprHasVar(node: ExprNode | null): boolean {
    if (!node) return false;
    if (node.kind === "var") return true;
    if (node.kind === "cast") return exprHasVar(node.expr);
    if (node.kind === "unary") return exprHasVar(node.expr);
    if (node.kind === "postfix") return exprHasVar(node.expr);
    if (node.kind === "subscript")
      return exprHasVar(node.left) || exprHasVar(node.index);
    if (node.kind === "assign")
      return exprHasVar(node.left) || exprHasVar(node.right);
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
    const parseCastType = (): { targetType: string; nextIndex: number } | null => {
      if (
        !tokens[idx] ||
        tokens[idx]?.type !== "sym" ||
        tokens[idx]?.value !== "("
      ) {
        return null;
      }
      let at = idx + 1;
      const typeWords: string[] = [];
      while (
        at < tokens.length &&
        tokens[at]?.type === "kw" &&
        TYPE_KEYWORDS.has(tokens[at]!.value)
      ) {
        typeWords.push(tokens[at]!.value);
        at++;
      }
      if (!typeWords.length) return null;
      const base = canonicalizeBaseType(typeWords.join(" "));
      if (!base) return null;
      let depth = 0;
      while (
        at < tokens.length &&
        tokens[at]?.type === "sym" &&
        tokens[at]?.value === "*"
      ) {
        depth++;
        at++;
      }
      if (
        at >= tokens.length ||
        tokens[at]?.type !== "sym" ||
        tokens[at]?.value !== ")"
      ) {
        return null;
      }
      const targetType =
        makeSimplePointerType(depth, base) || `${base}${"*".repeat(depth)}`;
      return { targetType, nextIndex: at + 1 };
    };
    const isAssignmentOperator = (value: string): value is AssignmentOp =>
      value === "=" ||
      value === "+=" ||
      value === "-=" ||
      value === "*=" ||
      value === "/=" ||
      value === "%=" ||
      value === "<<=" ||
      value === ">>=" ||
      value === "&=" ||
      value === "^=" ||
      value === "|=";

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
        const expr = parseAssignment();
        if (!expr) return null;
        const close = next();
        if (!close || close.type !== "sym" || close.value !== ")") return null;
        idx++;
        return expr;
      }
      return null;
    }

    function parsePostfix(): ExprNode | null {
      let left = parsePrimary();
      if (!left) return null;
      while (true) {
        const tok = next();
        if (!tok || tok.type !== "sym") break;
        if (tok.value === "[") {
          idx++;
          const index = parseAssignment();
          if (!index) return null;
          const close = next();
          if (!close || close.type !== "sym" || close.value !== "]") return null;
          idx++;
          left = { kind: "subscript", left, index };
          continue;
        }
        if (tok.value === "++" || tok.value === "--") {
          idx++;
          left = { kind: "postfix", op: tok.value, expr: left };
          continue;
        }
        break;
      }
      return left;
    }

    function parseUnary(): ExprNode | null {
      const cast = parseCastType();
      if (cast) {
        idx = cast.nextIndex;
        const expr = parseUnary();
        if (!expr) return null;
        return { kind: "cast", targetType: cast.targetType, expr };
      }
      const tok = next();
      if (
        tok &&
        tok.type === "sym" &&
        (tok.value === "+" ||
          tok.value === "-" ||
          tok.value === "!" ||
          tok.value === "~" ||
          tok.value === "*" ||
          tok.value === "&" ||
          tok.value === "++" ||
          tok.value === "--")
      ) {
        idx++;
        const expr = parseUnary();
        if (!expr) return null;
        return { kind: "unary", op: tok.value as UnaryOp, expr };
      }
      return parsePostfix();
    }

    function parseMulDiv(): ExprNode | null {
      let left = parseUnary();
      if (!left) return null;
      while (true) {
        const tok = next();
        if (
          !tok ||
          tok.type !== "sym" ||
          (tok.value !== "*" && tok.value !== "/" && tok.value !== "%")
        )
          break;
        idx++;
        const right = parseUnary();
        if (!right) return null;
        left = { kind: "binary", op: tok.value as BinaryOp, left, right };
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
        left = { kind: "binary", op: tok.value as BinaryOp, left, right };
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
        left = { kind: "binary", op: tok.value as BinaryOp, left, right };
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
        left = { kind: "binary", op: tok.value as BinaryOp, left, right };
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
        left = { kind: "binary", op: tok.value as BinaryOp, left, right };
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
        left = { kind: "binary", op: tok.value as BinaryOp, left, right };
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
        left = { kind: "binary", op: tok.value as BinaryOp, left, right };
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
        left = { kind: "binary", op: tok.value as BinaryOp, left, right };
      }
      return left;
    }

    function parseLogicalAnd(): ExprNode | null {
      let left = parseBitwiseOr();
      if (!left) return null;
      while (true) {
        const tok = next();
        if (!tok || tok.type !== "sym" || tok.value !== "&&") break;
        idx++;
        const right = parseBitwiseOr();
        if (!right) return null;
        left = { kind: "binary", op: tok.value as BinaryOp, left, right };
      }
      return left;
    }

    function parseLogicalOr(): ExprNode | null {
      let left = parseLogicalAnd();
      if (!left) return null;
      while (true) {
        const tok = next();
        if (!tok || tok.type !== "sym" || tok.value !== "||") break;
        idx++;
        const right = parseLogicalAnd();
        if (!right) return null;
        left = { kind: "binary", op: tok.value as BinaryOp, left, right };
      }
      return left;
    }

    function parseAssignment(): ExprNode | null {
      const left = parseLogicalOr();
      if (!left) return null;
      const tok = next();
      if (!tok || tok.type !== "sym" || !isAssignmentOperator(tok.value)) {
        return left;
      }
      idx++;
      const right = parseAssignment();
      if (!right) return null;
      return { kind: "assign", op: tok.value, left, right };
    }

    const expr = parseAssignment();
    if (!expr) return null;
    return { expr, nextIndex: idx, hasVar: exprHasVar(expr) };
  }

  function isExpressionPrefix(
    tokens: Token[],
    { allowVars = true }: { allowVars?: boolean } = {},
  ): boolean {
    if (!tokens.length) return true;
    let expectingOperand = true;
    let parenDepth = 0;
    let subscriptDepth = 0;
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
            parenDepth++;
            continue;
          }
          if (
            tok.value === "+" ||
            tok.value === "-" ||
            tok.value === "~" ||
            tok.value === "!" ||
            tok.value === "*" ||
            tok.value === "&" ||
            tok.value === "++" ||
            tok.value === "--"
          ) {
            continue;
          }
        }
        return false;
      } else {
        if (tok.type === "sym") {
          if (tok.value === "++" || tok.value === "--") {
            continue;
          }
          if (tok.value === "[") {
            subscriptDepth++;
            expectingOperand = true;
            continue;
          }
          if (tok.value === "]") {
            if (subscriptDepth <= 0) return false;
            subscriptDepth--;
            continue;
          }
          if (tok.value === ")") {
            if (parenDepth <= 0) return false;
            parenDepth--;
            continue;
          }
          if (
            tok.value === "+" ||
            tok.value === "-" ||
            tok.value === "*" ||
            tok.value === "/" ||
            tok.value === "%" ||
            tok.value === "==" ||
            tok.value === "!=" ||
            tok.value === "<<" ||
            tok.value === ">>" ||
            tok.value === "<" ||
            tok.value === "<=" ||
            tok.value === ">" ||
            tok.value === ">=" ||
            tok.value === "&&" ||
            tok.value === "||" ||
            tok.value === "&" ||
            tok.value === "^" ||
            tok.value === "|" ||
            tok.value === "=" ||
            tok.value === "+=" ||
            tok.value === "-=" ||
            tok.value === "*=" ||
            tok.value === "/=" ||
            tok.value === "%=" ||
            tok.value === "<<=" ||
            tok.value === ">>=" ||
            tok.value === "&=" ||
            tok.value === "^=" ||
            tok.value === "|="
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

  function parseArrayLengthTokens(tokens: Token[]): number | null {
    if (!tokens.length) return null;
    const parsed = parseExpressionTokens(tokens, 0, { allowVars: false });
    if (!parsed || parsed.nextIndex !== tokens.length) return null;
    const length = evaluateArrayLengthExpr(parsed.expr);
    if (!Number.isFinite(length) || (length || 0) <= 0) return null;
    return Number(length);
  }

  function parseDeclarator(
    tokens: Token[],
    start: number,
  ):
    | {
        kind: "ok";
        name: string;
        opsOutward: DeclDerivedOp[];
        nextIndex: number;
      }
    | { kind: "partial" }
    | { kind: "none" } {
    let idx = start;
    let pointerCount = 0;
    while (idx < tokens.length && tokens[idx]?.type === "sym" && tokens[idx]?.value === "*") {
      pointerCount++;
      idx++;
    }

    const parseDirect = (
      from: number,
    ):
      | {
          kind: "ok";
          name: string;
          opsOutward: DeclDerivedOp[];
          nextIndex: number;
        }
      | { kind: "partial" }
      | { kind: "none" } => {
      let at = from;
      let name = "";
      let opsOutward: DeclDerivedOp[] = [];
      const first = tokens[at];
      if (!first) return { kind: "partial" };
      if (first.type === "ident") {
        name = first.value;
        at++;
      } else if (first.type === "sym" && first.value === "(") {
        const inner = parseDeclarator(tokens, at + 1);
        if (inner.kind !== "ok") return inner;
        const close = tokens[inner.nextIndex];
        if (!close) return { kind: "partial" };
        if (close.type !== "sym" || close.value !== ")") return { kind: "none" };
        name = inner.name;
        opsOutward = inner.opsOutward.slice();
        at = inner.nextIndex + 1;
      } else {
        return { kind: "none" };
      }

      while (at < tokens.length && tokens[at]?.type === "sym" && tokens[at]?.value === "[") {
        at++;
        const lengthTokens: Token[] = [];
        let bracketDepth = 1;
        while (at < tokens.length) {
          const tok = tokens[at]!;
          if (tok.type === "sym" && tok.value === "[") {
            bracketDepth++;
            lengthTokens.push(tok);
            at++;
            continue;
          }
          if (tok.type === "sym" && tok.value === "]") {
            bracketDepth--;
            if (bracketDepth === 0) break;
            lengthTokens.push(tok);
            at++;
            continue;
          }
          lengthTokens.push(tok);
          at++;
        }
        if (at >= tokens.length || bracketDepth !== 0) return { kind: "partial" };
        if (!lengthTokens.length) return { kind: "partial" };
        const length = parseArrayLengthTokens(lengthTokens);
        if (!Number.isFinite(length) || (length || 0) <= 0) return { kind: "none" };
        opsOutward.push({ kind: "array", length: Number(length) });
        at++; // ]
      }

      return { kind: "ok", name, opsOutward, nextIndex: at };
    };

    const direct = parseDirect(idx);
    if (direct.kind !== "ok") return direct;
    const opsOutward = direct.opsOutward.slice();
    for (let i = 0; i < pointerCount; i++) opsOutward.push({ kind: "ptr" });
    return {
      kind: "ok",
      name: direct.name,
      opsOutward,
      nextIndex: direct.nextIndex,
    };
  }

  function parseDeclHead(tokens: Token[]): DeclHeadParseResult {
    if (!tokens.length) return { kind: "none" };
    if (tokens[0]?.type !== "kw" || !TYPE_KEYWORDS.has(tokens[0].value)) {
      return { kind: "none" };
    }
    let idx = 0;
    const typeWords: string[] = [];
    while (
      idx < tokens.length &&
      tokens[idx]?.type === "kw" &&
      TYPE_KEYWORDS.has(tokens[idx]!.value)
    ) {
      typeWords.push(tokens[idx]!.value);
      idx++;
    }
    const base = canonicalizeBaseType(typeWords.join(" "));
    if (!base) return { kind: "none" };

    if (idx >= tokens.length) return { kind: "partial" };
    const declarator = parseDeclarator(tokens, idx);
    if (declarator.kind !== "ok") return declarator;
    idx = declarator.nextIndex;
    const fullType = buildDeclType(base, declarator.opsOutward);
    const peeled = peelArrayType(fullType);
    const loweredElement = lowerDeclType(peeled.element);
    if (!loweredElement) return { kind: "none" };
    const elementType = formatDeclType(loweredElement);
    if (!elementType) return { kind: "none" };

    const loweredFull = loweredElement;
    const declType = formatDeclType(loweredFull);
    if (!declType) return { kind: "none" };

    const resultBase = {
      kind: "full" as const,
      name: declarator.name,
      hasInitializer: false,
      declType,
      pointeeArrayDims: loweredFull.pointeeArrayDims,
      pointeeInnerDepth: loweredFull.innerPointerDepth,
      arrayShape: peeled.shape,
      elementType,
      elementPointeeArrayDims: loweredElement.pointeeArrayDims,
      elementPointeeInnerDepth: loweredElement.innerPointerDepth,
    };

    if (idx >= tokens.length) return resultBase;
    const eqTok = tokens[idx];
    if (!eqTok || eqTok.type !== "sym" || eqTok.value !== "=") return { kind: "none" };
    idx++;
    if (idx >= tokens.length) {
      return { kind: "partial", name: declarator.name, hasInitializer: true };
    }
    return {
      ...resultBase,
      hasInitializer: true,
      rhsStart: idx,
    };
  }

  function isDeclPrefix(tokens: Token[]): boolean {
    const parsed = parseDeclHead(tokens);
    if (parsed.kind === "none") return false;
    if (parsed.kind === "partial") return true;
    if (!parsed.hasInitializer) return true;
    if (!Number.isFinite(parsed.rhsStart)) return true;
    return isExpressionPrefix(tokens.slice(parsed.rhsStart!));
  }

  function isAssignPrefix(tokens: Token[]): boolean {
    return isExpressionPrefix(tokens, { allowVars: true });
  }

  function isConditionalPrefix(tokens: Token[], keyword: "if" | "while"): boolean {
    if (!tokens.length) return false;
    if (tokens[0].type !== "kw" || tokens[0].value !== keyword) return false;
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

  function isIfPrefix(tokens: Token[]): boolean {
    return isConditionalPrefix(tokens, "if");
  }

  function isWhilePrefix(tokens: Token[]): boolean {
    return isConditionalPrefix(tokens, "while");
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
        TYPE_KEYWORDS.has(t0.value)
      )
        return true;
      if (t0.type === "sym" && (t0.value === "{" || t0.value === "}"))
        return true;
      if (t0.type === "ident") {
        if (
          allowIntPrefix &&
          ("signed".startsWith(t0.value) ||
            "unsigned".startsWith(t0.value) ||
            "short".startsWith(t0.value) ||
            "long".startsWith(t0.value) ||
            "int".startsWith(t0.value) ||
            "char".startsWith(t0.value) ||
            "float".startsWith(t0.value) ||
            "double".startsWith(t0.value) ||
            "_Bool".startsWith(t0.value) ||
            "bool".startsWith(t0.value) ||
            "if".startsWith(t0.value) ||
            "while".startsWith(t0.value) ||
            "else".startsWith(t0.value))
        )
          return true;
        return hasDeclaredPrefix(t0.value, declaredNames);
      }
      if (t0.type === "sym" && (t0.value === "*" || t0.value === "&")) return true;
    }
    return (
      isIfPrefix(tokens) ||
      isWhilePrefix(tokens) ||
      isDeclPrefix(tokens) ||
      isAssignPrefix(tokens)
    );
  }

  function parseAssignRhs(
    tokens: Token[],
    idx: number,
  ): ExprParseResult | null {
    if (idx >= tokens.length) return null;
    const parsed = parseExpressionTokens(tokens, idx, { allowVars: true });
    if (!parsed || parsed.nextIndex !== tokens.length) return null;
    return { expr: parsed.expr, hasVar: parsed.hasVar, nextIndex: parsed.nextIndex };
  }

  function parseConditionHeaderTokens(
    tokens: Token[],
    keyword: "if" | "while",
  ): { expr: ExprNode; hasVar: boolean } | null {
    if (!tokens.length) return null;
    if (tokens[0].type !== "kw" || tokens[0].value !== keyword) return null;
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

  function parseIfHeaderTokens(
    tokens: Token[],
  ): { expr: ExprNode; hasVar: boolean } | null {
    return parseConditionHeaderTokens(tokens, "if");
  }

  function parseWhileHeaderTokens(
    tokens: Token[],
  ): { expr: ExprNode; hasVar: boolean } | null {
    return parseConditionHeaderTokens(tokens, "while");
  }

  function parseStatementTokens(tokens: Token[]): Statement | null {
    if (!tokens.length) return null;
    if (tokens.length === 1 && tokens[0].type === "sym") {
      if (tokens[0].value === "{") return { kind: "blockStart" };
      if (tokens[0].value === "}") return { kind: "blockEnd" };
      if (tokens[0].value === ";") return { kind: "empty" };
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
    const whileParsed = parseWhileHeaderTokens(tokens);
    if (whileParsed) {
      return { kind: "while", expr: whileParsed.expr, hasVar: whileParsed.hasVar };
    }
    const declHead = parseDeclHead(tokens);
    if (declHead.kind === "full" && declHead.declType && declHead.name) {
      if (Array.isArray(declHead.arrayShape) && declHead.arrayShape.length > 0) {
        if (declHead.hasInitializer) return null;
        const shape = declHead.arrayShape.map((d) => Math.max(0, Math.floor(Number(d))));
        const elementType = declHead.elementType || declHead.declType;
        return {
          kind: "decl",
          name: declHead.name,
          type: elementType,
          arrayShape: shape,
          elementType,
          elementPointeeArrayDims: declHead.elementPointeeArrayDims || [],
          elementPointeeInnerDepth: declHead.elementPointeeInnerDepth,
          declaredNames: makeArrayDeclaredNames(declHead.name, shape),
        };
      }
      if (!declHead.hasInitializer) {
        return {
          kind: "decl",
          name: declHead.name,
          type: declHead.declType,
          pointeeArrayDims: declHead.pointeeArrayDims || [],
          pointeeInnerDepth: declHead.pointeeInnerDepth,
          declaredNames: [declHead.name],
        };
      }
      const rhsStart = Number.isFinite(declHead.rhsStart) ? declHead.rhsStart! : -1;
      if (rhsStart < 0) return null;
      const rhs = parseAssignRhs(tokens, rhsStart);
      if (!rhs) return null;
      return {
        kind: "declAssign",
        name: declHead.name,
        declType: declHead.declType,
        pointeeArrayDims: declHead.pointeeArrayDims || [],
        pointeeInnerDepth: declHead.pointeeInnerDepth,
        expr: rhs.expr,
        hasVar: rhs.hasVar,
        declaredNames: [declHead.name],
      };
    }
    const parsedExpr = parseExpressionTokens(tokens, 0, { allowVars: true });
    if (parsedExpr && parsedExpr.nextIndex === tokens.length) {
      if (parsedExpr.expr.kind === "assign") {
        return {
          kind: "assign",
          op: parsedExpr.expr.op,
          lhs: parsedExpr.expr.left,
          rhs: parsedExpr.expr.right,
          hasVar: parsedExpr.hasVar,
        };
      }
      return {
        kind: "expr",
        expr: parsedExpr.expr,
        hasVar: parsedExpr.hasVar,
      };
    }
    return null;
  }

  function controlHeaderEndIndex(tokens: Token[]): number {
    if (!tokens.length) return -1;
    if (tokens[0].type !== "kw") return -1;
    if (tokens[0].value !== "if" && tokens[0].value !== "while") return -1;
    if (tokens.length < 2) return -1;
    if (tokens[1].type !== "sym" || tokens[1].value !== "(") return -1;
    let depth = 0;
    for (let i = 1; i < tokens.length; i++) {
      const tok = tokens[i];
      if (tok.type === "sym" && tok.value === "(") {
        depth++;
        continue;
      }
      if (tok.type === "sym" && tok.value === ")") {
        depth--;
        if (depth < 0) return -1;
        if (depth === 0) return i;
      }
    }
    return -1;
  }

  function splitStatements(tokens: Token[]): StatementPart[] {
    const parts: StatementPart[] = [];
    let current: Token[] = [];
    let startLine = 0;
    const pushCurrent = (endLine: number, hasSemicolon: boolean) => {
      if (!current.length) return;
      parts.push({
        tokens: current,
        startLine: current[0]?.line ?? startLine,
        endLine,
        hasSemicolon,
      });
      current = [];
    };
    for (const tok of tokens) {
      if (current.length) {
        const splitAfterElse =
          current.length === 1 &&
          current[0].type === "kw" &&
          current[0].value === "else" &&
          !(tok.type === "sym" && tok.value === "{");
        const headerEnd = controlHeaderEndIndex(current);
        const splitAfterControlHeader =
          headerEnd >= 0 && headerEnd === current.length - 1;
        if (splitAfterElse || splitAfterControlHeader) {
          pushCurrent(current[current.length - 1].line, false);
          startLine = tok.line;
        }
      }
      if (tok.type === "sym" && tok.value === ";") {
        if (current.length) {
          parts.push({
            tokens: current,
            startLine: current[0]?.line ?? startLine,
            endLine: tok.line,
            hasSemicolon: true,
          });
          current = [];
        } else {
          parts.push({
            tokens: [tok],
            startLine: tok.line,
            endLine: tok.line,
            hasSemicolon: true,
          });
        }
        startLine = tok.line;
        continue;
      }
      if (isBraceToken(tok)) {
        pushCurrent(current[current.length - 1]?.line ?? tok.line, false);
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
    pushCurrent(current[current.length - 1]?.line ?? startLine, false);
    return parts;
  }

  return {
    tokenizeProgram,
    parseExpressionTokens,
    parseDeclHead,
    parseIfHeaderTokens,
    parseWhileHeaderTokens,
    parseStatementTokens,
    isStatementPrefix,
    controlHeaderEndIndex,
    splitStatements,
  };
}
