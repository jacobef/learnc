import { C_INTERPRETER_WASM_BASE64 } from "./shared-c-interpreter-wasm-data.js";
let exportsCache = null;
const SYNTHETIC_ADDRESS_BASE_STORAGE_KEY = `cboxes-synthetic-address-base-v2:${window.location.pathname}`;
const MIN_SYNTHETIC_ADDRESS_BASE = 1000;
const MAX_SYNTHETIC_ADDRESS_BASE = 9000;
const SYNTHETIC_ADDRESS_ALIGNMENT = 16;
const DEFAULT_EXECUTION_BUDGET = {
    stepLimit: 10000,
    followingTraceLimit: 256,
};
function normalizeExecutionBudget(budget) {
    return {
        stepLimit: Math.max(1, Math.floor(budget.stepLimit)),
        followingTraceLimit: Math.max(1, Math.floor(budget.followingTraceLimit)),
    };
}
export function createSyntheticAddressBase() {
    const random = new Uint32Array(1);
    crypto.getRandomValues(random);
    const firstAlignedBase = Math.ceil(MIN_SYNTHETIC_ADDRESS_BASE / SYNTHETIC_ADDRESS_ALIGNMENT) *
        SYNTHETIC_ADDRESS_ALIGNMENT;
    const baseCount = Math.floor((MAX_SYNTHETIC_ADDRESS_BASE - firstAlignedBase) /
        SYNTHETIC_ADDRESS_ALIGNMENT) + 1;
    return (firstAlignedBase +
        (random[0] % baseCount) * SYNTHETIC_ADDRESS_ALIGNMENT);
}
function loadSyntheticAddressBase() {
    try {
        const stored = Number.parseInt(localStorage.getItem(SYNTHETIC_ADDRESS_BASE_STORAGE_KEY) ?? "", 10);
        if (Number.isSafeInteger(stored) &&
            stored >= MIN_SYNTHETIC_ADDRESS_BASE &&
            stored <= MAX_SYNTHETIC_ADDRESS_BASE &&
            stored % SYNTHETIC_ADDRESS_ALIGNMENT === 0) {
            return stored;
        }
        const generated = createSyntheticAddressBase();
        localStorage.setItem(SYNTHETIC_ADDRESS_BASE_STORAGE_KEY, generated.toString());
        return generated;
    }
    catch {
        return createSyntheticAddressBase();
    }
}
const syntheticAddressBase = loadSyntheticAddressBase();
class WasiProcExit extends Error {
    constructor(code) {
        super(`interpreter exited through WASI proc_exit(${code})`);
        this.name = "WasiProcExit";
        this.code = code;
    }
}
function decodeBase64(data) {
    const binary = atob(data);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i += 1) {
        bytes[i] = binary.charCodeAt(i);
    }
    return bytes;
}
function encodeSourceFiles(files) {
    const encoder = new TextEncoder();
    const encoded = files.map((file) => ({
        path: encoder.encode(file.path),
        source: encoder.encode(file.source),
    }));
    const byteLength = 4 +
        encoded.reduce((total, file) => total + 8 + file.path.length + file.source.length, 0);
    const output = new Uint8Array(byteLength);
    const view = new DataView(output.buffer);
    let offset = 0;
    view.setUint32(offset, encoded.length, true);
    offset += 4;
    for (const file of encoded) {
        view.setUint32(offset, file.path.length, true);
        view.setUint32(offset + 4, file.source.length, true);
        offset += 8;
        output.set(file.path, offset);
        offset += file.path.length;
        output.set(file.source, offset);
        offset += file.source.length;
    }
    return output;
}
function writeU32(memory, ptr, value) {
    new DataView(memory.buffer).setUint32(ptr, value, true);
}
function writeU64(memory, ptr, value) {
    new DataView(memory.buffer).setBigUint64(ptr, value, true);
}
function wasiImports(getExports) {
    const errnoNosys = 52;
    const errnoNotsup = 58;
    const ok = 0;
    return {
        clock_time_get: (clockId, _precision, timePtr) => {
            const memory = getExports()?.memory;
            if (!memory)
                return errnoNosys;
            const nanoseconds = clockId === 0
                ? BigInt(Date.now()) * 1000000n
                : clockId === 1
                    ? BigInt(Math.floor(performance.now() * 1000000))
                    : null;
            if (nanoseconds === null)
                return errnoNotsup;
            writeU64(memory, timePtr, nanoseconds);
            return ok;
        },
        environ_get: () => ok,
        environ_sizes_get: (countPtr, sizePtr) => {
            const memory = getExports()?.memory;
            if (memory) {
                writeU32(memory, countPtr, 0);
                writeU32(memory, sizePtr, 0);
            }
            return ok;
        },
        fd_close: () => ok,
        fd_seek: () => errnoNosys,
        fd_write: (_fd, _iovs, _iovsLen, nwrittenPtr) => {
            const memory = getExports()?.memory;
            if (memory)
                writeU32(memory, nwrittenPtr, 0);
            return ok;
        },
        proc_exit: (code) => {
            throw new WasiProcExit(code);
        },
        random_get: (ptr, len) => {
            const memory = getExports()?.memory;
            if (!memory)
                return errnoNosys;
            const bytes = new Uint8Array(memory.buffer, ptr, len);
            for (let offset = 0; offset < bytes.length; offset += 65536) {
                crypto.getRandomValues(bytes.subarray(offset, offset + 65536));
            }
            return ok;
        },
    };
}
function interpreterExports() {
    if (exportsCache)
        return exportsCache;
    let current = null;
    const wasmBytes = decodeBase64(C_INTERPRETER_WASM_BASE64);
    const moduleBytes = Uint8Array.from(wasmBytes);
    const module = new WebAssembly.Module(moduleBytes);
    const instance = new WebAssembly.Instance(module, {
        env: {
            clock: () => Math.floor(Date.now() / 1000),
        },
        wasi_snapshot_preview1: wasiImports(() => current),
    });
    current = instance.exports;
    exportsCache = current;
    return current;
}
function sourceLines(source) {
    const lines = String(source || "").split(/\r?\n/);
    return lines.length ? lines : [""];
}
function clampLine(line, lines) {
    return Math.max(0, Math.min(lines.length - 1, Math.floor(line)));
}
function lineEndColumn(lines, line) {
    return Math.max(0, lines[clampLine(line, lines)]?.length ?? 0);
}
function lineAt(lines, line) {
    return lines[clampLine(line, lines)] ?? "";
}
function lastAssignmentOperatorBefore(text, beforeColumn) {
    for (let index = Math.min(beforeColumn - 1, text.length - 1); index >= 0; index -= 1) {
        if (text[index] !== "=")
            continue;
        const previous = text[index - 1] || "";
        const next = text[index + 1] || "";
        if (previous === "=" || next === "=" || previous === "!" || previous === "<" || previous === ">") {
            continue;
        }
        return index;
    }
    return -1;
}
function destinationRangeBeforeExpression(lines, primary) {
    for (let line = primary.startLine; line >= Math.max(0, primary.startLine - 6); line -= 1) {
        const text = lineAt(lines, line);
        if (line < primary.startLine && /[;{}]\s*$/.test(text))
            return null;
        const beforeColumn = line === primary.startLine ? primary.startCol : text.length;
        const equal = lastAssignmentOperatorBefore(text, beforeColumn);
        if (equal < 0)
            continue;
        const boundary = Math.max(text.lastIndexOf(";", equal - 1), text.lastIndexOf("{", equal - 1), text.lastIndexOf("}", equal - 1), text.lastIndexOf(",", equal - 1));
        const rawLeft = text.slice(boundary + 1, equal);
        const leadingSpace = rawLeft.length - rawLeft.trimStart().length;
        const left = rawLeft.trim();
        if (!left || /^return\b/.test(left))
            return null;
        const leftStart = boundary + 1 + leadingSpace;
        const declaration = /^(?:(?:const|volatile|restrict|static|extern|register|auto|signed|unsigned|short|long|_Atomic)\s+)*(?:(?:struct|union|enum)\s+[A-Za-z_]\w*|(?:void|char|int|float|double|_Bool|bool|wchar_t|char16_t|char32_t|[A-Za-z_]\w*))\s*\*+?\s*([A-Za-z_]\w*)\s*(?:\[[^\]]*\]\s*)*$/.exec(left)
            || /^(?:(?:const|volatile|restrict|static|extern|register|auto|signed|unsigned|short|long|_Atomic)\s+)*(?:(?:struct|union|enum)\s+[A-Za-z_]\w*|(?:void|char|int|float|double|_Bool|bool|wchar_t|char16_t|char32_t|[A-Za-z_]\w*))\s+([A-Za-z_]\w*)\s*(?:\[[^\]]*\]\s*)*$/.exec(left);
        if (declaration) {
            const name = declaration[1];
            const nameOffset = left.lastIndexOf(name);
            return {
                startLine: line,
                startCol: leftStart + nameOffset,
                endLine: line,
                endCol: leftStart + nameOffset + name.length,
            };
        }
        return {
            startLine: line,
            startCol: leftStart,
            endLine: line,
            endCol: leftStart + left.length,
        };
    }
    return null;
}
function linkedDiagnosticMessageParts(message, hasDestination) {
    const links = [];
    const primary = /\b(?:This|this) (?:expression|value|string literal)\b/.exec(message)
        || /^([A-Za-z_]\w*)(?= is (?:a|an|not)\b)/.exec(message);
    if (primary?.index != null) {
        links.push({
            start: primary.index,
            end: primary.index + primary[0].length,
            annotationId: "primary",
        });
    }
    if (hasDestination) {
        const destination = /\b(?:(?:This|this) location|member [A-Za-z_]\w*(?:\.[A-Za-z_]\w*)*)\b/.exec(message);
        if (destination?.index != null) {
            links.push({
                start: destination.index,
                end: destination.index + destination[0].length,
                annotationId: "destination",
            });
        }
    }
    if (!links.length)
        return undefined;
    links.sort((left, right) => left.start - right.start);
    const parts = [];
    let cursor = 0;
    for (const link of links) {
        if (link.start < cursor)
            continue;
        if (link.start > cursor)
            parts.push({ text: message.slice(cursor, link.start) });
        parts.push({
            text: message.slice(link.start, link.end),
            annotationId: link.annotationId,
        });
        cursor = link.end;
    }
    if (cursor < message.length)
        parts.push({ text: message.slice(cursor) });
    return parts;
}
function previousCodeLine(lines, line) {
    for (let index = Math.min(line - 1, lines.length - 1); index >= 0; index -= 1) {
        if ((lines[index] || "").trim())
            return index;
    }
    return clampLine(line - 1, lines);
}
function unmatchedOpeningBraceCount(source) {
    let depth = 0;
    let inLineComment = false;
    let inBlockComment = false;
    let quote = null;
    let escaped = false;
    for (let index = 0; index < source.length; index += 1) {
        const ch = source[index] || "";
        const next = source[index + 1] || "";
        if (inLineComment) {
            if (ch === "\n")
                inLineComment = false;
            continue;
        }
        if (inBlockComment) {
            if (ch === "*" && next === "/") {
                inBlockComment = false;
                index += 1;
            }
            continue;
        }
        if (quote) {
            if (escaped) {
                escaped = false;
            }
            else if (ch === "\\") {
                escaped = true;
            }
            else if (ch === quote) {
                quote = null;
            }
            continue;
        }
        if (ch === "/" && next === "/") {
            inLineComment = true;
            index += 1;
            continue;
        }
        if (ch === "/" && next === "*") {
            inBlockComment = true;
            index += 1;
            continue;
        }
        if (ch === "'" || ch === '"') {
            quote = ch;
            continue;
        }
        if (ch === "{")
            depth += 1;
        if (ch === "}")
            depth = Math.max(0, depth - 1);
    }
    return depth;
}
function commonTypeSuggestion(name) {
    if (name === "integer")
        return "C uses int for whole numbers. Try writing int instead of integer.";
    if (name === "number")
        return "C does not have a type named number. Use int for whole numbers or double for decimals.";
    if (name === "string")
        return "C does not have a beginner-friendly string type here. Use char arrays or char pointers when the tutorial introduces them.";
    if (name === "boolean")
        return "C uses bool only after including <stdbool.h>. In these lessons, use int values like 0 and 1 for false and true.";
    if (name === "doubl")
        return "Did you mean double? C needs the full type name.";
    return null;
}
function declarationLooksLikeUnknownType(line, column, name) {
    if (line.slice(column, column + name.length) !== name)
        return false;
    const afterName = line.slice(column + name.length);
    return /^\s+[A-Za-z_]\w*/.test(afterName)
        || /^\s*\*+\s*(?:(?:const|volatile|restrict)\s+)*[A-Za-z_]\w*/.test(afterName)
        || /^\s*\(\s*\*+\s*[A-Za-z_]\w*/.test(afterName);
}
function sourceBeforePosition(lines, line, column) {
    return [
        ...lines.slice(0, line),
        lineAt(lines, line).slice(0, Math.max(0, column)),
    ].join("\n");
}
function stripCommentsAndLiterals(source) {
    return source.replace(/\/\*[\s\S]*?(?:\*\/|$)|\/\/[^\n]*|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'/g, (match) => match.replace(/[^\n]/g, " "));
}
function visibleTagKeywordBefore(lines, line, column, name) {
    const escapedName = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const pattern = new RegExp(`\\b(struct|union|enum)\\s+${escapedName}\\b`, "g");
    const source = stripCommentsAndLiterals(sourceBeforePosition(lines, line, column));
    let keyword = null;
    for (const match of source.matchAll(pattern)) {
        keyword = match[1];
    }
    return keyword;
}
function unknownTypeNameDiagnostic(lines, line, column, name) {
    const tagKeyword = visibleTagKeywordBefore(lines, line, column, name);
    if (tagKeyword) {
        const article = tagKeyword === "enum" ? "an" : "a";
        return `${name} is ${article} ${tagKeyword} tag, not a type name by itself. Write ${tagKeyword} ${name} instead of ${name} here.`;
    }
    return `${name} is not a type name C recognizes here. Check the spelling. If you intended to define a new type name, declare it with typedef first.`;
}
function lineLooksLikeForgottenComma(line) {
    return /^\s*(?:const\s+)?(?:unsigned\s+|signed\s+)?(?:int|double|float|char|short|long|bool)\s+\*?\s*[A-Za-z_]\w*\s+[A-Za-z_]\w*/.test(line);
}
function declaredAsPointerBefore(lines, beforeLine, name) {
    const escaped = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const declaration = new RegExp(`\\b(?:const\\s+)?(?:unsigned\\s+|signed\\s+)?(?:int|double|float|char|short|long|bool|void)\\s*\\*+\\s*${escaped}\\b`);
    for (let index = 0; index <= Math.min(beforeLine, lines.length - 1); index += 1) {
        if (declaration.test(lines[index] || ""))
            return true;
    }
    return false;
}
function dereferencedAssignmentName(line) {
    const match = /^\s*\*\s*([A-Za-z_]\w*)\s*=/.exec(line);
    return match?.[1] ?? null;
}
function friendlyDiagnosticFor(message, source, line, col) {
    const lines = sourceLines(source);
    const safeLine = clampLine(line, lines);
    const text = lineAt(lines, safeLine);
    const textAtDiagnostic = text.slice(Math.max(0, col)).trimStart();
    const atEnd = line >= lines.length;
    const diagnosticOffset = lines
        .slice(0, safeLine)
        .reduce((offset, sourceLine) => offset + sourceLine.length + 1, 0)
        + Math.min(Math.max(0, col), text.length);
    const memberPath = [];
    let memberMessage = message;
    while (true) {
        const memberContext = /^while initializing member ([A-Za-z_]\w*): (.+)$/.exec(memberMessage);
        if (!memberContext)
            break;
        memberPath.push(memberContext[1]);
        memberMessage = memberContext[2];
    }
    if (memberPath.length) {
        const friendly = friendlyDiagnosticFor(memberMessage, source, line, col);
        const memberLabel = `member ${memberPath.join(".")}`;
        const conversion = /^cannot convert (.+) to (.+)$/.exec(memberMessage);
        const contextualMessage = conversion
            ? `This expression has type ${conversion[1]}, but ${memberLabel} has type ${conversion[2]}.`
            : /\b(?:This|this) location\b/.test(friendly.message)
                ? friendly.message.replace(/\b(?:This|this) location\b/, memberLabel)
                : `While initializing ${memberLabel}: ${friendly.message}`;
        return { ...friendly, message: contextualMessage };
    }
    if (/^expected ';', found /.test(message)) {
        if (/\d+\.\.\d*|\d+\.\d+\./.test(text)) {
            return { message: "This number has too many decimal points. Use one decimal point, like 1.2." };
        }
        if (/\band\b/.test(text)) {
            return { message: "C uses && for logical and. It does not use the word and here." };
        }
        if (/\bor\b/.test(text)) {
            return { message: "C uses || for logical or. It does not use the word or here." };
        }
        if (/\bint\s+mian\s*\(/.test(text)) {
            return { message: "Did you mean main? cBoxes only treats a function named main as the program entry point." };
        }
        if (/\b(?:int|double|float|char|short|long|bool)\s+[A-Za-z_]\w*\s+(?:\d|')/.test(text)) {
            return { message: "C expected the declaration to end here. If the following value is an initializer, put = before it; otherwise remove the extra value." };
        }
        if (/^\s*(?:int|double|float|char|short|long|void)\s+[A-Za-z_]\w*\s*\{/.test(text)) {
            return { message: "Function definitions need parentheses after the function name, like int f(void) { ... }." };
        }
        if (/\b(?:int|double|float|char|short|long|bool)\s+[A-Za-z_]\w*-[A-Za-z_]\w*/.test(text)) {
            return { message: "C variable names cannot contain hyphens. Use an underscore instead, like my_var." };
        }
        if (text[col] === ")" || /\)\s*;/.test(text.slice(Math.max(0, col - 1)))) {
            return { message: "There is an extra closing parenthesis ')' here." };
        }
        if (lineLooksLikeForgottenComma(text)) {
            return {
                message: "C expected this declaration to end here. If you meant to declare another variable, put a comma before its name, like int a, b;.",
            };
        }
        if (!text.slice(0, Math.max(0, col)).trim() && safeLine > 0) {
            const previous = previousCodeLine(lines, safeLine);
            return {
                message: "The previous statement is missing a semicolon (;). Add ; at the end of that line.",
                line: previous,
                col: lineEndColumn(lines, previous),
            };
        }
        if (atEnd || col <= 0) {
            const previous = previousCodeLine(lines, line);
            return {
                message: "The previous statement is missing a semicolon (;). Add ; at the end of that line.",
                line: previous,
                col: lineEndColumn(lines, previous),
            };
        }
        return {
            message: "Add a semicolon (;) here to end the statement.",
            col,
        };
    }
    if (/^expected ';' before continuing in another source file$/.test(message)) {
        return {
            message: "This declaration is missing a semicolon (;). Add ; at the end of this line.",
            col: lineEndColumn(lines, safeLine),
        };
    }
    if (/^expected '\(', found /.test(message)) {
        const keyword = /\bwhile\b/.test(text) ? "while" : /\bif\b/.test(text) ? "if" : null;
        if (keyword) {
            return {
                message: `Put the ${keyword} condition in parentheses, like ${keyword} (condition) { ... }.`,
            };
        }
        return { message: "Add an opening parenthesis '(' here." };
    }
    if (/^expected '\)', found /.test(message)) {
        if (/^expected '\)', found keyword /.test(message) && /\([^)]*\b(?:int|char|float|double|short|long|void|struct)\b[^,)]*\b(?:int|char|float|double|short|long|void|struct)\b/.test(text)) {
            return { message: "C expected the parameter list to end here. If another parameter follows, put a comma before its type." };
        }
        if (/^expected '\)', found (?:number|identifier) /.test(message)) {
            return { message: "C expected a closing parenthesis here. If these are separate function arguments, put a comma between them." };
        }
        if (/\b(if|while)\b/.test(text)) {
            return {
                message: "Add a closing parenthesis ')' after the condition, before the opening brace.",
            };
        }
        return { message: "Add a closing parenthesis ')' here." };
    }
    if (/^expected '\]', found /.test(message)) {
        return { message: "Add a closing bracket ']' here." };
    }
    if (/^expected '\{', found /.test(message)) {
        if (/^expected '\{', found '\)'$/.test(message)) {
            return { message: "There is an extra closing parenthesis ')' here." };
        }
        if (/\)\s*=/.test(text)) {
            return {
                message: "A function cannot have an initializer after =. End a declaration with ;, or replace = and the value with a function body in { braces }.",
            };
        }
        return { message: "Add an opening brace '{' to start this block." };
    }
    if (/^expected '}', found /.test(message)) {
        if (/^expected '}', found (?:number|identifier|a character literal|a string literal)/.test(message)
            && text.slice(0, Math.max(0, col)).includes("{")) {
            return { message: "Put a comma before this value to separate it from the previous initializer." };
        }
        if (/=\s*\{/.test(text) || /=\s*\{[\s\S]*$/.test(source.slice(0, diagnosticOffset))) {
            return { message: "Add a closing brace '}' to finish this initializer before the semicolon." };
        }
        if (/\benum\b[^{}]*\{/.test(text)) {
            return { message: "Add a closing brace '}' to finish this enum definition." };
        }
        return { message: "Add a closing brace '}' to end this block." };
    }
    if (/^expected the end of the file, found '}'$/.test(message)) {
        return { message: "This closing brace '}' does not match an opening brace. Remove it or add the missing opening brace earlier." };
    }
    if (/^expected ',', found /.test(message)) {
        return { message: "C expected a comma here to separate these items." };
    }
    if (/^expected ':', found /.test(message)) {
        return { message: "C expected a colon ':' here." };
    }
    if (/^expected '=', found /.test(message)) {
        return { message: "C expected an equals sign '=' here." };
    }
    if (/^expected identifier$/.test(message)) {
        if (/\b(?:const\s+)?(?:unsigned\s+|signed\s+)?(?:int|double|float|char|short|long|bool)\s+\d/.test(text)) {
            return { message: "Variable names cannot start with a number. Start the name with a letter or underscore." };
        }
        if (/,\s*;/.test(text)) {
            return {
                message: "After a comma in a declaration, write another variable name, or remove the comma.",
            };
        }
        return { message: "C expected a name here. Use a variable, function, member, or label name." };
    }
    if (/^expected type specifier$/.test(message)) {
        return { message: "C expected a type here, such as int, double, char, a typedef name, or struct followed by a tag or definition." };
    }
    if (/^expected member name after \.$/.test(message)) {
        return { message: "Write a struct or union member name after '.', such as point.x." };
    }
    if (/^expected member name after ->$/.test(message)) {
        return { message: "Write a struct or union member name after '->', such as point_pointer->x." };
    }
    if (/^expected label name after goto$/.test(message)) {
        return { message: "Write the destination label name after goto, such as goto cleanup;." };
    }
    if (/^expected enumerator name$/.test(message)) {
        return { message: "Each item in an enum needs a name, such as enum Color { RED, GREEN, BLUE };." };
    }
    if (/^expected expression$/.test(message)) {
        if (/===/.test(text)) {
            return { message: "C uses == to compare values. It does not have JavaScript's === operator." };
        }
        if (/\b(?:else\s+)?(?:if|while)\s*\(\s*\)/.test(text)) {
            return { message: "Put a condition between the parentheses." };
        }
        if (/\b(?:if|while)\s*\([^)]*\)\s*:/.test(text)) {
            return { message: "C does not use a colon after if or while. Use braces: if (condition) { ... }." };
        }
        if (/\bif\s*\([^)]*\)\s*else\b/.test(text)) {
            return { message: "An if needs a statement or block before else. Usually you want if (condition) { ... } else { ... }." };
        }
        if (/\bsizeof\s+(?:int|double|float|char|short|long|bool)\b/.test(text)) {
            return { message: "When using sizeof with a type, put the type in parentheses, like sizeof(int)." };
        }
        if (/^else\b/.test(textAtDiagnostic) || /^\s*else\b/.test(text)) {
            return { message: "else must come right after an if block. Check that the if and its braces come before this else." };
        }
        if (/=\s*;/.test(text)) {
            return { message: "Put a value or expression after the equals sign, before the semicolon." };
        }
        if (/[+\-*/%<>=!&|]\s*;/.test(text)) {
            return { message: "This operator needs a value or expression on its right side." };
        }
        if (/[+\-*/%<>=!&|?:]\s*$/.test(source.slice(0, diagnosticOffset).trimEnd())) {
            return { message: "This operator needs a value or expression on its right side." };
        }
        if (unmatchedOpeningBraceCount(source) > 0 && (atEnd || safeLine === lines.length - 1)) {
            return {
                message: "A block is missing a closing brace '}'. Add } to close the block before the program ends.",
                line: lines.length - 1,
                col: lineEndColumn(lines, lines.length - 1),
            };
        }
        return { message: "C expected a value or expression here." };
    }
    if (/^struct or union definition is missing a closing brace$/.test(message)) {
        return { message: "This struct or union definition is missing its closing brace '}'. Add } before the program ends." };
    }
    if (/^struct or union definition is missing \} and ; before this function definition$/.test(message)) {
        return { message: "This function appears inside a struct or union definition. Add }; before the function to close the type definition." };
    }
    if (/^an integer constant expression cannot use a floating-point value$/.test(message)) {
        return { message: "This value must be an integer constant, so it cannot contain a decimal point or floating-point exponent." };
    }
    if (/^zero-width bit-field must be unnamed$/.test(message)) {
        return { message: "A bit-field with width 0 cannot have a name. Remove the name, or use a width greater than 0." };
    }
    const duplicateMember = /^duplicate member declaration ([A-Za-z_]\w*)$/.exec(message);
    if (duplicateMember) {
        return { message: `This struct or union already has a member named ${duplicateMember[1]}. Rename or remove one member.` };
    }
    if (/^record members cannot have void type$/.test(message)) {
        return { message: "A struct or union member cannot have type void because void has no values. Give the member an object type such as int." };
    }
    if (/^record members cannot have function type$/.test(message)) {
        return { message: "A struct or union cannot contain a function as a member. Store a function pointer instead, or define the function outside the type." };
    }
    if (/^a function parameter must have complete object type after adjustment$/.test(message)) {
        if (/\bvoid\s+[A-Za-z_]\w*/.test(textAtDiagnostic)) {
            return { message: "A parameter cannot be a named void value. Write void by itself for no parameters, or give this parameter a type such as int." };
        }
        return { message: "This parameter's type is incomplete, so C does not know how to pass it by value. Define the type first, or pass a pointer to it." };
    }
    if (/^unsupported conditional operand types$/.test(message)) {
        return { message: "The values after ? and : have incompatible types. Make both alternatives produce compatible values." };
    }
    const objectConflict = /^conflicting declarations of object ([A-Za-z_]\w*): the earlier declaration has type (.+), but this one has type (.+)$/.exec(message);
    if (objectConflict) {
        return { message: `${objectConflict[1]} was previously declared with type ${objectConflict[2]}, but this declaration gives it type ${objectConflict[3]}. Use the same type in both declarations.` };
    }
    const functionConflict = /^conflicting declarations of function ([A-Za-z_]\w*): the earlier declaration has type (.+), but this one has type (.+)$/.exec(message);
    if (functionConflict) {
        return { message: `${functionConflict[1]} was previously declared with type ${functionConflict[2]}, but this declaration gives it type ${functionConflict[3]}. Make its return and parameter types match.` };
    }
    if (/^function parameter list is missing a closing parenthesis$/.test(message)) {
        return { message: "This function's parameter list is missing a closing parenthesis ')'. Add ) before the function body." };
    }
    if (/^array bound must be an integer constant expression in this context$/.test(message)) {
        return { message: "This array size must be an integer value known at compile time." };
    }
    if (/^bit-field width must be non-negative$/.test(message)) {
        return { message: "A bit-field width cannot be negative. Use 0 for an unnamed separator, or a positive width for a named member." };
    }
    if (/^_Alignof requires a complete object type$/.test(message)) {
        return { message: "_Alignof cannot measure this type because the type has not been fully defined." };
    }
    if (/^__builtin_offsetof requires a struct or union type$/.test(message)) {
        const offsetOf = /\boffsetof\s*\(\s*([^,]+)/.exec(text);
        if (offsetOf) {
            const typeText = offsetOf[1].trim();
            const typeStart = offsetOf.index + offsetOf[0].lastIndexOf(offsetOf[1])
                + offsetOf[1].indexOf(typeText);
            return {
                message: "offsetof needs a struct or union type as its first argument.",
                col: typeStart,
                endCol: typeStart + typeText.length,
            };
        }
        return { message: "offsetof needs a struct or union type as its first argument." };
    }
    if (/^_Generic association type must be a complete object type$/.test(message)) {
        return { message: "This _Generic association uses an incomplete type. Define the struct or union first, or use a complete object type." };
    }
    if (/^_Generic associations may not specify compatible types more than once$/.test(message)) {
        return { message: "This _Generic type duplicates an earlier compatible association. Keep only one association for that type." };
    }
    if (/^_Generic may specify default at most once$/.test(message)) {
        return { message: "A _Generic selection can have only one default choice. Remove or combine the duplicate default." };
    }
    if (/^requested alignment is not supported$/.test(message)) {
        return { message: "This alignment is not supported. Use 0 or a supported power-of-two alignment such as 1, 2, 4, 8, or 16." };
    }
    if (/^_Alignas cannot request an alignment weaker than the type's natural alignment$/.test(message)) {
        return { message: "_Alignas cannot reduce a type's normal alignment. Remove _Alignas or request a larger supported alignment." };
    }
    if (/^variable length array objects cannot have static storage duration$/.test(message)) {
        return { message: "A static array needs a size known at compile time. Use a constant size, or remove static so this array can use a runtime size." };
    }
    if (/^variable length array bound evaluated to a non-positive value$/.test(message)) {
        return { message: "This array size evaluated to zero or a negative number. Make sure its runtime size is greater than zero." };
    }
    const noGenericMatch = /^_Generic has no association compatible with controlling type (.+)$/.exec(message);
    if (noGenericMatch) {
        return { message: `This _Generic selection has no choice for type ${noGenericMatch[1]}. Add a compatible type association or a default choice.` };
    }
    const incompleteCompoundLiteral = /^compound literal has incomplete type (.+)$/.exec(message);
    if (incompleteCompoundLiteral) {
        return { message: `This compound literal uses incomplete type ${incompleteCompoundLiteral[1]}. Define that type before creating a value of it.` };
    }
    if (/^expected macro name after defined\($/.test(message)) {
        return { message: "Write a macro name inside defined(...), such as defined(FEATURE)." };
    }
    if (/^structure with a flexible array member must have at least one other named member$/.test(message)) {
        return { message: "A struct with an empty [ ] array member needs at least one ordinary named member before that array." };
    }
    const invalidCast = /^invalid cast from (.+) to (.+)$/.exec(message);
    if (invalidCast) {
        return { message: `A value of type ${invalidCast[1]} cannot be converted to ${invalidCast[2]} with a cast.` };
    }
    const nonConstantIdentifier = /^identifier ([A-Za-z_]\w*) is not an integer constant expression$/.exec(message);
    if (nonConstantIdentifier) {
        if (/\bcase\b/.test(text)) {
            return { message: `A case label must be an integer value known at compile time. ${nonConstantIdentifier[1]} is a variable, so it cannot be used here.` };
        }
        if (/\[\s*[A-Za-z_]\w*\s*\]\s*=/.test(text)) {
            return { message: `An initializer's [index] must be an integer value known at compile time. ${nonConstantIdentifier[1]} is a variable.` };
        }
        return { message: `${nonConstantIdentifier[1]} is a variable, but this location requires an integer value known at compile time.` };
    }
    if (/^a function cannot return an array type$/.test(message)) {
        return { message: "A C function cannot return an array directly. Return a pointer, or pass an output array to the function." };
    }
    if (/^a function cannot return a function type$/.test(message)) {
        return { message: "A C function cannot return another function directly. Return a function pointer instead." };
    }
    if (/^expression is not a supported integer constant expression$/.test(message) && /\{\s*\[/.test(text)) {
        return { message: "An array initializer designator has the form [integer_index] = value. Check the closing ] and the = after it." };
    }
    if (/^expected while after do-body$/.test(message)) {
        return { message: "A do block must be followed by while (condition);. Add while and its condition, or use a different loop." };
    }
    const undeclared = /^use of undeclared identifier ([A-Za-z_]\w*)$/.exec(message);
    if (undeclared) {
        const name = undeclared[1] || "";
        if (new RegExp(`=\\s*\\{[^}]*\\b${name}\\s*=`).test(text)) {
            return { message: `${name} has not been declared. If it is a struct or union member designator, write .${name} = value; otherwise declare ${name} before using it.` };
        }
        const typeSuggestion = commonTypeSuggestion(name);
        if (typeSuggestion)
            return { message: typeSuggestion };
        if (name === "Int")
            return { message: "C type names are lowercase. Write int, not Int." };
        if (name === "let" || name === "var") {
            return { message: "C declares variables with a type instead of let or var. Write something like int a = 3;." };
        }
        if (name === "not")
            return { message: "C uses ! for logical not. It does not use the word not here." };
        if (name === "and")
            return { message: "C uses && for logical and. It does not use the word and here." };
        if (name === "or")
            return { message: "C uses || for logical or. It does not use the word or here." };
        if (name === "print") {
            return { message: "C does not have Python-style print(...). Use printf(...) with #include <stdio.h>, or just assign values to variables in these lessons." };
        }
        if (name === "printf")
            return { message: "To use printf, add #include <stdio.h> at the top of the program." };
        if (name === "scanf")
            return { message: "To use scanf, add #include <stdio.h> and make sure the variables you read into are declared." };
        if (name === "malloc")
            return { message: "To use malloc, add #include <stdlib.h>. In the tutorial levels, you usually do not need malloc." };
        if (name === "true" || name === "false") {
            return { message: "In these lessons, use 1 for true and 0 for false. C bool values require #include <stdbool.h>." };
        }
        if (declarationLooksLikeUnknownType(text, col, name)) {
            return {
                message: unknownTypeNameDiagnostic(lines, safeLine, col, name),
            };
        }
        if (new RegExp(`\\b${name}\\s*\\(`).test(text)) {
            return {
                message: `No function named ${name} has been declared. Check the spelling, include its header, or declare the function before calling it.`,
            };
        }
        return {
            message: `No declaration for ${name} is visible here. Check the spelling, or declare it before this use.`,
        };
    }
    if (/^unsupported #include syntax$/.test(message)) {
        return {
            message: "This #include is incomplete. Write a header name like #include <stdio.h>, or remove the include.",
        };
    }
    if (/^empty header name in #include$/.test(message)) {
        return {
            message: "Put a header name between < and >, like #include <stdio.h>, or remove the include.",
        };
    }
    if (/^unterminated character constant$/.test(message)) {
        return { message: "This character literal is missing its closing single quote (')." };
    }
    if (/unterminated quoted literal/.test(message) && text.includes("'") && !text.includes('"')) {
        return { message: "This character literal is missing its closing single quote (')." };
    }
    if (/unterminated quoted literal/.test(message) || /^unterminated string literal$/.test(message)) {
        if (/^\s*#\s*include/.test(text)) {
            return { message: "This #include is missing the closing quote or > for the header name." };
        }
        return { message: "This string is missing its closing double quote (\")." };
    }
    if (/^empty character constant$/.test(message)) {
        return { message: "A character literal needs one character between the single quotes, like 'a'." };
    }
    const unknownEscape = /^unknown escape sequence (\\.)$/.exec(message);
    if (unknownEscape) {
        return {
            message: `C does not recognize ${unknownEscape[1]} as an escape sequence. Use a supported escape such as \\n, \\t, \\\\, \\" or remove the backslash.`,
        };
    }
    if (/^unsupported integer literal suffix$/.test(message)) {
        return { message: "This integer has an invalid suffix. Remove the suffix, or use a valid combination of U for unsigned and L or LL for long integer types." };
    }
    if (/^invalid hexadecimal integer literal$/.test(message)) {
        return { message: "A hexadecimal integer starts with 0x and needs at least one digit from 0-9 or a-f after it." };
    }
    if (/^invalid octal integer literal$/.test(message)) {
        return { message: "An integer beginning with 0 is octal and may use only digits 0 through 7. Remove the leading 0 for an ordinary decimal number." };
    }
    if (/^invalid integer literal$/.test(message)) {
        return { message: "This is not a valid C integer. Check its digits, base prefix, and suffix." };
    }
    if (/^integer literal is out of supported range$/.test(message)) {
        return { message: "This integer literal is too large for every supported C integer type. Use a smaller value or a calculation that stays in range." };
    }
    if (/^unterminated escape sequence$/.test(message)) {
        return { message: "This literal ends immediately after a backslash. Complete the escape sequence or remove the backslash." };
    }
    if (/^u and U character constants must contain exactly one character$/.test(message)) {
        return { message: "A prefixed character literal such as u'a' or U'a' must contain exactly one character. Use double quotes for text containing multiple characters." };
    }
    if (/^(?:character is not representable|numeric escape sequence is outside the range) in the literal's character type$/.test(message)) {
        return { message: "This character or numeric escape does not fit in the literal's character type. Use an appropriate u, U, or L prefix, or choose a representable value." };
    }
    if (/^unterminated block comment/.test(message)) {
        return { message: "This block comment is missing its closing */." };
    }
    const unexpected = /^unexpected character (.+)$/.exec(message);
    if (unexpected) {
        return { message: `C does not use ${unexpected[1]} here. Remove it or replace it with the right C operator or punctuation.` };
    }
    if (/^the & operator requires an object or function; a temporary value does not have an address$/.test(message)) {
        if (/&\s*(?:\d|')/.test(text)) {
            return { message: "The & operator takes the address of a variable. A number or character literal does not have an address you can use here." };
        }
        return { message: "The & operator can only take the address of a variable, array element, dereferenced pointer, or function." };
    }
    if (/^(?:object type must be complete(?: for a definition)?|object definition has incomplete type .+)$/.test(message) && /\[\s*\]/.test(text)) {
        return { message: "C needs to know the array size here. Write a size inside the brackets, like int a[3];." };
    }
    const redefinition = /^redefinition of ([A-Za-z_]\w*)$/.exec(message);
    if (redefinition) {
        return {
            message: `${redefinition[1]} has already been declared in this scope. Remove one declaration or give one of them a different name.`,
        };
    }
    if (/^an object cannot have void type$/.test(message)) {
        return { message: "A variable cannot have type void because void has no values. Choose a type such as int, or use void only as a function return type or in void*." };
    }
    const invalidObjectType = /^an object cannot have type (.+)$/.exec(message);
    if (invalidObjectType) {
        if (invalidObjectType[1] === "void") {
            return { message: "A variable cannot have type void because void has no values. Choose a type that can store a value, such as int." };
        }
        if (invalidObjectType[1].startsWith("function(")) {
            return { message: "A variable cannot have a function type. Declare a function pointer if this variable should store a function address." };
        }
        return { message: `A variable cannot have type ${invalidObjectType[1]}. Choose an object type that can store a value.` };
    }
    if (/^array bound must be greater than zero$/.test(message)) {
        return { message: "This array size must be greater than zero." };
    }
    if (/^array bound must have integer type$/.test(message)) {
        return { message: "An array size must be a whole-number integer expression, not a floating-point value." };
    }
    if (/^variable length array objects cannot have an initializer$/.test(message)) {
        return {
            message: "This array's size is determined while the program runs, so C cannot initialize it with = here. Declare the array first, then assign its elements.",
        };
    }
    if (/^compound literal cannot have variable length array type$/.test(message)) {
        return { message: "An array compound literal needs a size known at compile time. Use a constant bound, or declare the runtime-sized array separately and fill its elements." };
    }
    const incompatibleTypeSpecifiers = /^(?:float|double|char|void|_Bool|_Complex) cannot be combined with /.exec(message);
    if (incompatibleTypeSpecifiers) {
        return { message: "These type words do not form a valid C type. Remove the type word that does not belong." };
    }
    const duplicateType = /^duplicate type specifier (.+)$/.exec(message);
    if (duplicateType) {
        return { message: `The type word ${duplicateType[1]} appears more than once. Remove the duplicate.` };
    }
    if (/^multiple storage class specifiers are not allowed$/.test(message)) {
        return { message: "A declaration can use only one storage-class word such as static, extern, auto, register, or typedef. Remove the conflicting one." };
    }
    if (/^restrict qualifier requires a pointer type$/.test(message)) {
        return { message: "restrict can qualify only a pointer. Remove restrict, or put * in the declaration if this object is meant to be a pointer." };
    }
    if (/^block-scope extern declaration cannot have an initializer$/.test(message)) {
        return { message: "An extern declaration inside a function refers to an object defined elsewhere, so it cannot have an initializer here. Remove the initializer or define a normal local variable." };
    }
    if (/^typedef declaration cannot have an initializer$/.test(message)) {
        return { message: "A typedef creates a type name, not a variable, so it cannot have an initializer. Remove the = value." };
    }
    if (/^initializer for static storage duration object is not a compile-time constant$/.test(message)) {
        if (/\bstatic\b/.test(text)) {
            return {
                message: "A static local variable must be initialized with a compile-time constant. Remove static if the initializer must call a function or use another runtime value.",
            };
        }
        return {
            message: "A global variable must be initialized with a compile-time constant. Move the runtime initialization into a function, or use a constant initializer here.",
        };
    }
    const directAndBuiltinType = /^(.+) cannot be combined with a built-in type specifier$/.exec(message);
    if (directAndBuiltinType) {
        if (/}\s*$/.test(text)) {
            return {
                message: "A struct, union, or enum definition must end with a semicolon. Add ; after the closing brace.",
                col: lineEndColumn(lines, safeLine),
            };
        }
        return { message: `${directAndBuiltinType[1]} is already a complete type specifier, so it cannot be combined with a built-in type such as int or double.` };
    }
    if (/^type specifier cannot be both signed and unsigned$/.test(message)) {
        return { message: "A type cannot be both signed and unsigned. Keep only the one that describes the range you want." };
    }
    if (/^short cannot be combined with long$/.test(message)) {
        return { message: "A type cannot be both short and long. Remove one of those size specifiers." };
    }
    const unknownSizeType = /^sizeof cannot determine the size of type (.+)$/.exec(message);
    if (/^sizeof requires a complete non-void object type$/.test(message) || unknownSizeType) {
        const type = unknownSizeType?.[1];
        if (type === "void" || /sizeof\s*\(\s*void\s*\)/.test(text)) {
            return { message: "void has no size because it has no values. sizeof cannot be applied to void." };
        }
        if (type?.startsWith("function(")) {
            return { message: "A function is executable code, not an object with a size that sizeof can measure." };
        }
        if (type) {
            if (/\[0\]$/.test(type)) {
                return { message: "This array type has no known positive size, so sizeof cannot measure it. Give the array a positive bound first." };
            }
            const incompleteRecord = /\b(struct|union|enum)\s+([A-Za-z_]\w*)\b/.exec(type);
            if (incompleteRecord) {
                const namedType = `${incompleteRecord[1]} ${incompleteRecord[2]}`;
                return { message: `${namedType} has not been defined yet, so C does not know its size. Define ${namedType} before using sizeof on it.` };
            }
            return { message: `${type} is incomplete here, so sizeof cannot determine its size. Complete that type's definition first.` };
        }
        return { message: "sizeof cannot determine this type's size because the type is incomplete here." };
    }
    const incompleteObject = /^object definition has incomplete type (.+)$/.exec(message);
    if (incompleteObject) {
        const type = incompleteObject[1];
        if (/\[0\]$/.test(type)) {
            return { message: "This array type has no known positive size. Write a positive size inside its brackets before declaring the array." };
        }
        const incompleteRecord = /\b(struct|union|enum)\s+([A-Za-z_]\w*)\b/.exec(type);
        if (incompleteRecord) {
            const namedType = `${incompleteRecord[1]} ${incompleteRecord[2]}`;
            return { message: `${namedType} has been declared but not defined, so C does not know its size. Define ${namedType} before declaring an object of this type.` };
        }
        return { message: `${type} is incomplete here, so C does not know this object's size. Complete that type's definition before declaring the object.` };
    }
    if (/^object type must be complete for a definition$/.test(message)) {
        return { message: "This object's type is incomplete here, so C does not know its size. Complete the type before declaring the object." };
    }
    const incompleteMember = /^record member has incomplete type (.+)$/.exec(message);
    if (incompleteMember) {
        const type = incompleteMember[1];
        const incompleteRecord = /\b(struct|union|enum)\s+([A-Za-z_]\w*)\b/.exec(type);
        if (incompleteRecord) {
            const namedType = `${incompleteRecord[1]} ${incompleteRecord[2]}`;
            return { message: `This member has incomplete type ${namedType}, so C does not know how much space the member needs. Define ${namedType} first, or make this a pointer member if it should refer to a ${namedType} object instead of containing one.` };
        }
        return { message: `This member has incomplete type ${type}, so C does not know how much space it needs. Complete the type before declaring the member.` };
    }
    if (/^record members must have complete type$/.test(message)) {
        return { message: "This member's type is incomplete, so C does not know how much space the member needs." };
    }
    const incompleteArrayElement = /^array element has incomplete type (.+)$/.exec(message);
    if (incompleteArrayElement) {
        const type = incompleteArrayElement[1];
        if (/^function\b/.test(type)) {
            return { message: "An array cannot contain functions as elements. Use an array of function pointers instead." };
        }
        if (type === "void") {
            return { message: "An array cannot have void elements because void has no values. Choose an object type such as int." };
        }
        if (/\[0\]$/.test(type)) {
            return { message: "This array's element type is itself an array with no positive size. Give the inner array dimension a positive bound." };
        }
        const incompleteRecord = /\b(struct|union|enum)\s+([A-Za-z_]\w*)\b/.exec(type);
        if (incompleteRecord) {
            const namedType = `${incompleteRecord[1]} ${incompleteRecord[2]}`;
            return { message: `This array would contain ${namedType} elements, but ${namedType} has not been defined yet. Define ${namedType} before declaring the array.` };
        }
        return { message: `This array's element type, ${type}, is incomplete. Complete that type before declaring the array.` };
    }
    if (/^flexible array member must be the last member of a structure$/.test(message)) {
        return { message: "An array member written with empty [ ] must be the final member of its struct. Move it to the end or give it a fixed size." };
    }
    const emptyRecord = /^(struct|union) must declare at least one member$/.exec(message);
    if (emptyRecord) {
        return { message: `C11 does not allow an empty ${emptyRecord[1]}. Add at least one member.` };
    }
    if (/^structs and unions must declare at least one member$/.test(message)) {
        return { message: "C11 does not allow this empty record definition. Add at least one member." };
    }
    const incompleteAnonymousMember = /^anonymous member has incomplete type (.+)$/.exec(message);
    if (incompleteAnonymousMember) {
        return { message: `This anonymous member has incomplete type ${incompleteAnonymousMember[1]}. Define that type before embedding it as an anonymous member.` };
    }
    const namelessDeclaration = /^declaration of type (.+) requires a name$/.exec(message);
    if (namelessDeclaration) {
        return { message: `This ${namelessDeclaration[1]} declaration needs a name. Write the name after the type and before the semicolon.` };
    }
    const taglessRecord = /^(struct|union) type specifier requires a tag or a definition$/.exec(message);
    if (taglessRecord) {
        return { message: `Write a name after ${taglessRecord[1]}, or define the ${taglessRecord[1]}'s members inside { braces }.` };
    }
    const enumRedefinition = /^redefinition of enumerator ([A-Za-z_]\w*)$/.exec(message);
    if (enumRedefinition) {
        return { message: `The enum name ${enumRedefinition[1]} appears more than once. Each enumerator name must be unique in this scope.` };
    }
    if (/^call target is not a function$/.test(message)) {
        return { message: "The expression before ( ) is not a function or function pointer, so it cannot be called." };
    }
    if (/^function definition parameters must have names$/.test(message)) {
        return { message: "Every parameter in a function definition needs a name so the function body can use it, such as int f(int x)." };
    }
    if (/^function declaration is not allowed in this declaration context$/.test(message)) {
        return { message: "A function declaration cannot appear here. Move it outside the for initializer or parameter-declaration list, usually to file scope before the calling function." };
    }
    if (/^value of function call that reached the end of a non-void function is used$/.test(message)) {
        return { message: "This function promises to return a value, but this execution path reached its closing brace without return. Add a return value on every path." };
    }
    if (/^a variadic function requires at least one fixed parameter before \.\.\.$/.test(message)) {
        return { message: "A function using ... must declare at least one named parameter before it, such as int log(const char *format, ...)." };
    }
    if (/^the left side of = is not a stored object that can be changed$/.test(message)) {
        return { message: "The left side of = must name a place where C can store a value, such as a variable, array element, struct member, or *pointer." };
    }
    if (/^arrays cannot be assigned; assign their elements individually$/.test(message)) {
        return { message: "C cannot copy an array with =. Assign its elements individually, usually with a loop." };
    }
    if (/^the left side of = is const and cannot be changed$/.test(message)) {
        return { message: "This object was declared const, so its value cannot be changed after initialization." };
    }
    if (/^\+\+ and -- require a changeable arithmetic variable or pointer$/.test(message)) {
        return { message: "++ and -- can change only a numeric variable, pointer, array element, struct member, or dereferenced pointer—not a temporary calculation or const object." };
    }
    if (/^division by zero$/.test(message)) {
        return { message: "This divides by zero, which C does not allow." };
    }
    if (/^division by zero in constant expression$/.test(message)) {
        return { message: "This compile-time calculation divides by zero. Change the constant expression so its divisor is not zero." };
    }
    if (/^remainder with a zero divisor$/.test(message)) {
        return { message: "This uses % with zero on the right side, which C does not allow." };
    }
    if (/^dereference of a null pointer$/.test(message)) {
        return { message: "This pointer is null (0), so it does not point to a variable you can use." };
    }
    if (/^pointer is not valid to dereference$/.test(message) && /\[[^\]]+\]/.test(text)) {
        return { message: "This array index is outside the array's bounds. Use an index from 0 through one less than the array's length." };
    }
    if (/^pointer arithmetic produced a pointer outside the bounds of the object$/.test(message) && /\[[^\]]+\]/.test(text)) {
        return { message: "This array index is outside the array bounds." };
    }
    if (/^execution step limit reached$/.test(message)) {
        return { message: "The program ran for too many steps. Check for an infinite loop or recursion that never reaches its stopping condition." };
    }
    if (/^(?:read of a pointer value whose referent's lifetime has ended|(?:access|write|dereference) through? a pointer to an object whose lifetime has ended|dereference of a pointer to an object whose lifetime has ended)$/.test(message)) {
        const statementParts = text.slice(0, Math.max(0, col)).split(";");
        const currentStatementBeforeDiagnostic = statementParts[statementParts.length - 1] ?? "";
        if (/\bfree\s*\([^)]*$/.test(currentStatementBeforeDiagnostic)) {
            return { message: "This pointer has already been freed. Each allocated block must be passed to free at most once." };
        }
        return { message: "This pointer refers to an object that no longer exists. It may have been freed or may have belonged to a function or block that already ended." };
    }
    if (/^free requires a pointer value returned by malloc or a null pointer$/.test(message)) {
        return { message: "free can be used only with a pointer returned by malloc, calloc, or realloc (or with a null pointer). Do not free local variables, array storage, or string literals." };
    }
    if (/^attempt to modify a string literal or other read-only object$/.test(message)) {
        const indexedName = /\b([A-Za-z_]\w*)\s*\[/.exec(text)?.[1];
        const dereferencedName = /\*\s*([A-Za-z_]\w*)/.exec(text)?.[1];
        const name = indexedName ?? dereferencedName;
        if (name) {
            const escapedName = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
            const declaration = new RegExp(`\\bchar\\s*\\*\\s*${escapedName}\\s*=\\s*(\"(?:\\\\.|[^\"\\\\])*\")`).exec(lines.slice(0, safeLine + 1).join("\n"));
            if (declaration) {
                return {
                    message: `${name} points to the read-only string literal ${declaration[1]}. Change ${name}'s declaration to char ${name}[] = ${declaration[1]}; after that, ${name}[0] and the array's other characters can be changed.`,
                };
            }
        }
        return { message: "String literals are read-only. Declare the value you want to change as an array instead, then modify that array. For example: char s[] = \"hi\"; s[0] = 'a';" };
    }
    if (/^pointer subtraction requires pointers into the same array object or one past it$/.test(message)) {
        return { message: "Pointers can be subtracted only when both point into the same array (or just past its end). These pointers refer to different objects." };
    }
    if (/^pointer subtraction requires pointers to compatible complete object types$/.test(message)) {
        return { message: "Pointer subtraction requires pointers to the same element type with a known size. These pointers point to incompatible types." };
    }
    if (/^pointer arithmetic on a null pointer$/.test(message)) {
        return { message: "A null pointer does not point into an array, so it cannot be moved with +, -, ++, or --. Point it at an object first." };
    }
    const misalignedPointer = /^pointer conversion yields an? (.+) that is not correctly aligned$/.exec(message);
    if (misalignedPointer) {
        return { message: `This address is not correctly aligned for ${misalignedPointer[1]}. A pointer cast changes the type but cannot make a misaligned address safe to dereference.` };
    }
    if (/^left shift of a negative value is undefined$/.test(message)) {
        return { message: "The left operand of << is negative. Left-shifting a negative signed value is undefined in C." };
    }
    if (/^unsequenced (?:read|modification) of an object after it was modified$/.test(message)) {
        return { message: "This expression reads or changes the same object more than once without a defined order. Split the operations into separate statements." };
    }
    if (/^relational pointer comparison requires pointers into the same array object or one past it$/.test(message)) {
        return { message: "Ordering pointers with <, <=, >, or >= is defined only for positions in the same array. These pointers refer to different objects." };
    }
    if (/^use of an indeterminate pointer value$/.test(message)) {
        return { message: "This pointer no longer has a valid, usable value. It may be uninitialized or may point to a local object whose lifetime has ended." };
    }
    if (/^memcpy source and destination regions overlap$/.test(message)) {
        return { message: "memcpy requires source and destination regions that do not overlap. Use memmove when the regions may overlap." };
    }
    const overlappingStringOperation = /^(strcpy|strncpy|strcat|strncat|wcscpy|wcsncpy|wcscat|wcsncat) source and destination regions overlap$/.exec(message);
    if (overlappingStringOperation) {
        return { message: `${overlappingStringOperation[1]} requires separate source and destination regions. Copy through a separate buffer, or use an operation whose overlap rules fit the task.` };
    }
    const supportedRange = /^(.+) (?:size|byte count|field width|destination size|total byte count) is out of supported range$/.exec(message);
    if (supportedRange) {
        return { message: `The size supplied to ${supportedRange[1]} is too large for cBoxes to represent safely. Check for a negative value converted to size_t, integer overflow, or an unexpectedly large count.` };
    }
    if (/^null pointer passed where an object region is required$/.test(message)) {
        return { message: "This function needs a valid object or buffer, but it received a null pointer. Allocate or declare the object and pass its address." };
    }
    if (/^null pointer passed where a string or byte region is required$/.test(message)) {
        return { message: "This function needs a valid C string or byte buffer, but it received a null pointer." };
    }
    if (/^qsort comparison function pointer is null$/.test(message)) {
        return { message: "qsort needs a comparison function as its fourth argument. Pass the function name instead of a null pointer." };
    }
    if (/^fopen mode string must exactly match one of the standard mode sequences$/.test(message)) {
        return { message: "This fopen mode is invalid. Use a standard mode such as \"r\", \"w\", \"a\", \"rb\", or a valid form containing +." };
    }
    if (/^fclose requires a valid FILE \* designating an open stream$/.test(message)) {
        return { message: "fclose needs a non-null FILE* for a stream that is currently open. Check that fopen succeeded and that the stream was not already closed." };
    }
    const unsupportedPrintf = /^unsupported (printf|fprintf|sprintf|snprintf) conversion (%\S+)$/.exec(message);
    if (unsupportedPrintf) {
        return { message: `${unsupportedPrintf[1]} does not support the conversion ${unsupportedPrintf[2]}. Correct the conversion letter to match the value you want to print.` };
    }
    if (/^incomplete (printf|fprintf|sprintf|snprintf) format specifier$/.test(message)) {
        return { message: "The format string ends with an incomplete % conversion. Add the conversion letter, or write %% to print a percent sign." };
    }
    if (/^scanf destination array is too small for the converted multibyte sequence$/.test(message)) {
        return { message: "The input text does not fit in this destination array. Use a larger char array and a maximum field width, such as %9s for char text[10]." };
    }
    const byteAccess = /^requested byte access of (\d+) byte\(s\) starting at offset (\d+) exceeds the (\d+)-byte object$/.exec(message);
    if (byteAccess) {
        return { message: `This operation needs ${byteAccess[1]} bytes starting at offset ${byteAccess[2]}, but the destination object holds only ${byteAccess[3]} bytes. Increase the buffer size or copy less data.` };
    }
    if (/^string is not terminated within the bounds of its object$/.test(message)) {
        return { message: "This character array has no '\\0' terminator within its bounds, so it cannot be read as a C string. Add a terminator or use a length-limited operation." };
    }
    if (/^unsupported operands for pointer arithmetic$/.test(message) && /\[[^\]]*\d+\.\d+[^\]]*\]/.test(text)) {
        return { message: "Array indexes must be whole-number integer expressions, not decimals." };
    }
    const characterConstantToPointer = /^cannot convert a character constant to pointer type (.+)$/.exec(message);
    if (characterConstantToPointer) {
        if (/^(?:(?:const|volatile|restrict) )*char\*(?: (?:const|volatile|restrict))*$/.test(characterConstantToPointer[1])) {
            return { message: "This expression uses single quotes, which create a character constant, but this location needs a char* address. To supply a string, use double quotes. For example: char *s = \"a\";" };
        }
        return { message: `This expression uses single quotes, which create a character constant rather than an address. This location expects ${characterConstantToPointer[1]}. Use the address of a matching object instead.` };
    }
    const integerToPointer = /^cannot implicitly convert integer expression of type (.+) to pointer type (.+); the expression is not a null pointer constant$/.exec(message);
    if (integerToPointer) {
        return { message: `This expression has type ${integerToPointer[1]}, but this location expects ${integerToPointer[2]}.` };
    }
    if (/^equality comparison requires compatible pointer operand types$/.test(message)) {
        return { message: "These pointers point to different types, so C will not compare them directly. Make the pointer types match first." };
    }
    const noMember = /^(.+) has no member named ([A-Za-z_]\w*)$/.exec(message);
    if (noMember) {
        if (/\boffsetof\s*\(/.test(text) && !/^(?:struct|union)\b/.test(noMember[1])) {
            return { message: `offsetof needs a struct or union type as its first argument, but ${noMember[1]} is not one.` };
        }
        if (noMember[1].includes("[") && /\{\s*\./.test(text)) {
            return { message: `.${noMember[2]} selects a struct or union member, but this initializer is for an array. Use [index] to select an array element.` };
        }
        if (noMember[1].endsWith("*") && /\./.test(text)) {
            return { message: `${noMember[1]} is a pointer. Use ->${noMember[2]} to access the member, or dereference the pointer before using .${noMember[2]}.` };
        }
        return { message: `Type ${noMember[1]} has no member named ${noMember[2]}. Check the member's spelling and the struct or union definition.` };
    }
    const badDereference = /^the \* operator requires a pointer, but this expression has type (.+)$/.exec(message);
    if (badDereference) {
        if (/\*\*/.test(text)) {
            return { message: `C reads the second * as pointer dereferencing, but its operand has type ${badDereference[1]}. If you intended exponentiation, C has no ** operator; multiply explicitly or use pow from <math.h>.` };
        }
        if (/->/.test(text)) {
            return { message: `The -> operator needs a pointer to a struct or union, but this expression has type ${badDereference[1]}. If you have the struct object itself, use . instead.` };
        }
        return { message: `The * operator dereferences a pointer, but this expression has type ${badDereference[1]}. Use * only with a pointer value.` };
    }
    const badSubscript = /^array subscripting requires an array or pointer and an integer index, but the operands have types (.+) and (.+)$/.exec(message);
    if (badSubscript) {
        if (!badSubscript[1].endsWith("*") && !badSubscript[1].includes("[")) {
            return { message: `The value before [ has type ${badSubscript[1]}, not an array or pointer. Only arrays and pointers can be indexed.` };
        }
        return { message: `The index inside [ ] has type ${badSubscript[2]}. An array index must have an integer type.` };
    }
    const badArithmetic = /^this arithmetic operator requires numeric operands, but the operands have types (.+) and (.+)$/.exec(message);
    if (badArithmetic) {
        if (badArithmetic[1].endsWith("*") && badArithmetic[2].endsWith("*")) {
            return { message: "C cannot add or multiply two pointers. Pointer subtraction is allowed only for positions in the same array; otherwise combine a pointer with an integer offset." };
        }
        if (badArithmetic[1].endsWith("*") || badArithmetic[2].endsWith("*")) {
            return { message: `This operator cannot be applied to a pointer and a number. Only + and - can adjust a pointer by an integer offset.` };
        }
        return { message: `This arithmetic operator works on numbers, but its operands have types ${badArithmetic[1]} and ${badArithmetic[2]}.` };
    }
    const integerOperands = /^this operator requires integer operands, but the operands have types (.+) and (.+)$/.exec(message);
    if (integerOperands) {
        return { message: `This operator works only with integer values, but its operands have types ${integerOperands[1]} and ${integerOperands[2]}.` };
    }
    if (/^break statement is not within a loop or switch$/.test(message)) {
        return { message: "break only works inside a loop or switch. Move it into a loop, or remove it." };
    }
    if (/^continue statement is not within a loop$/.test(message)) {
        return { message: "continue only works inside a loop. Move it into a loop, or remove it." };
    }
    if (/^case\/default label is not within a switch statement$/.test(message)) {
        return { message: "case and default labels work only inside a switch statement. Move this label into a switch or remove it." };
    }
    if (/^switch expression must have integer type$/.test(message)) {
        return { message: "switch requires an integer or enum expression. It cannot switch directly on a floating-point value, string, pointer, or struct." };
    }
    const duplicateLabel = /^duplicate label ([A-Za-z_]\w*)$/.exec(message);
    if (duplicateLabel) {
        return { message: `The label ${duplicateLabel[1]} is defined more than once in this function. Rename or remove one definition.` };
    }
    if (/^goto enters the scope of an object with variably modified type$/.test(message)) {
        return { message: "This goto jumps past the declaration of an array whose size is computed at runtime. Move the label before that declaration or avoid the jump." };
    }
    const staticAssertion = /^static assertion failed: (.+)$/.exec(message);
    if (staticAssertion) {
        return { message: `This compile-time assertion failed: ${staticAssertion[1]}. Check the constant condition or the assumption described by its message.` };
    }
    if (/^_Static_assert requires a string literal message$/.test(message)) {
        return { message: "The second argument to _Static_assert must be text in double quotes, such as _Static_assert(condition, \"size is wrong\")." };
    }
    if (/^logical operators require scalar operands$/.test(message)) {
        return { message: "&& and || require numbers or pointers on both sides. A whole struct, union, or array cannot be used directly as a condition." };
    }
    if (/^expression of type (.+) is not scalar$/.test(message)) {
        return { message: "This condition or ! operand must be a number or pointer. A whole struct, union, or array cannot be used as true or false." };
    }
    if (/^invalid operands: equality comparison requires arithmetic operands, compatible pointer operand types, or a null pointer constant$/.test(message)) {
        return { message: "== and != can compare numbers or compatible pointers, but they cannot compare whole structs, unions, or unrelated pointer types." };
    }
    if (/^relational operators require real operands or pointers to compatible object types$/.test(message)) {
        return { message: "<, <=, >, and >= require real numbers or compatible pointers. They cannot order whole structs, unions, or complex values." };
    }
    if (/^shift operators require integer operands$/.test(message)) {
        return { message: "The << and >> operators work only with integer values, not floating-point numbers or pointers." };
    }
    const expectedArithmetic = /^expected arithmetic type, got (.+)$/.exec(message);
    if (expectedArithmetic) {
        return { message: `Unary + and - work only with numeric values, but this expression has type ${expectedArithmetic[1]}.` };
    }
    const expectedInteger = /^expected integer type, got (.+)$/.exec(message);
    if (expectedInteger) {
        return { message: `The ~ operator works only with integer values, but this expression has type ${expectedInteger[1]}.` };
    }
    const incompletePointerArithmetic = /^pointer arithmetic cannot be performed because pointed-to type (.+) is incomplete$/.exec(message);
    if (incompletePointerArithmetic) {
        return { message: `Pointer arithmetic needs the size of the pointed-to type, but ${incompletePointerArithmetic[1]} has an unknown size.` };
    }
    if (/^pointer arithmetic cannot be performed on void\*$/.test(message)) {
        return { message: "Pointer arithmetic is not allowed on void* because void has no size." };
    }
    if (/^pointer arithmetic cannot be performed on a function pointer$/.test(message)) {
        return { message: "Pointer arithmetic is not allowed on function pointers." };
    }
    const duplicateCase = /^duplicate case value (.+)$/.exec(message);
    if (duplicateCase) {
        return { message: `This switch already has a case for the value ${duplicateCase[1]}. Each case value must be unique.` };
    }
    if (/^multiple default labels$/.test(message)) {
        return { message: "A switch can have only one default label. Combine these default sections or remove one." };
    }
    const undeclaredLabel = /^use of undeclared label ([A-Za-z_]\w*)$/.exec(message);
    if (undeclaredLabel) {
        return { message: `goto refers to the label ${undeclaredLabel[1]}, but that label is not defined in this function. Check the spelling or add ${undeclaredLabel[1]}: before the destination statement.` };
    }
    if (/^a void function cannot return a value$/.test(message)) {
        return { message: "This function returns void, so write return; without a value, or change the function's return type." };
    }
    if (/^a non-void function must return a value in a return statement$/.test(message)) {
        return { message: "This function promises to return a value. Put a value after return, or change the function's return type to void." };
    }
    const argumentCount = /^function call expected (\d+)(\+?) argument\(s\), got (\d+)$/.exec(message);
    if (argumentCount) {
        const minimum = argumentCount[2] === "+";
        return {
            message: `This function call supplies ${argumentCount[3]} argument${argumentCount[3] === "1" ? "" : "s"}, but the function requires ${minimum ? "at least " : ""}${argumentCount[1]}. Check the function declaration and the comma-separated values inside ( ).`,
        };
    }
    const duplicateFunction = /^multiple definitions of function ([A-Za-z_]\w*)$/.exec(message);
    if (duplicateFunction) {
        return { message: `The function ${duplicateFunction[1]} has more than one body. Keep one definition and turn any others into declarations ending with ;.` };
    }
    const badMain = /^unsupported main signature: (.+)$/.exec(message);
    if (badMain) {
        if (badMain[1] === "main must return int") {
            return { message: "main must return int. Write int main(void) for a program that takes no command-line arguments." };
        }
        const parameterCount = /^this definition gives main (\d+) parameters?$/.exec(badMain[1]);
        if (parameterCount) {
            const count = Number(parameterCount[1]);
            return {
                message: `main cannot have ${count} parameter${count === 1 ? "" : "s"}. Write int main(void) for a program without command-line arguments, or int main(int argc, char **argv) when arguments are needed.`,
            };
        }
        return { message: `Use int main(void) for a program without command-line arguments, or int main(int argc, char **argv) when arguments are needed. The current declaration is invalid because ${badMain[1]}.` };
    }
    if (/^(?:translation unit|program) does not define main$/.test(message)) {
        return {
            message: "This program does not define main, the function where execution begins. Add a definition such as int main(void) { return 0; }.",
        };
    }
    const missingOldStyleParameterDeclaration = /^old-style parameter ([A-Za-z_]\w*) is missing its declaration$/.exec(message);
    if (missingOldStyleParameterDeclaration) {
        const name = missingOldStyleParameterDeclaration[1];
        return {
            message: `${name} has no type in this old-style function definition. Prefer putting a type before every parameter inside the parentheses, such as (int ${name}).`,
        };
    }
    const incompleteFunctionHeader = /^function header for ([A-Za-z_]\w*) is missing a body or semicolon$/.exec(message);
    if (incompleteFunctionHeader) {
        return {
            message: `The function header for ${incompleteFunctionHeader[1]} is incomplete. Add { ... } after it to define the function, or add ; to declare the function without a body.`,
        };
    }
    const missingDefinition = /^identifier ([A-Za-z_]\w*) is used in an expression, but the program does not provide a definition for it$/.exec(message);
    if (missingDefinition) {
        return { message: `${missingDefinition[1]} was declared but never defined. Add its function body or object definition to one of the project's .c files.` };
    }
    if (/^function call depth exceeded the interpreter limit of \d+; check for recursion that does not reach its base case$/.test(message)) {
        return { message: "Function calls nested too deeply. If this function is recursive, make sure every path eventually reaches a base case that does not recurse." };
    }
    const missingHeader = /^header file "([^"]+)" is not available$/.exec(message);
    if (missingHeader) {
        return { message: `The header ${missingHeader[1]} was not found. Add that file to the project, correct the include name, or use a supported standard header.` };
    }
    if (/^#else without a matching conditional group/.test(message)) {
        return { message: "This #else has no matching #if, #ifdef, or #ifndef above it. Add the opening directive or remove this #else." };
    }
    if (/^#endif without a matching conditional group/.test(message)) {
        return { message: "This #endif has no matching #if, #ifdef, or #ifndef above it. Add the opening directive or remove this #endif." };
    }
    if (/^#elif without a matching conditional group/.test(message)) {
        return { message: "This #elif has no matching #if, #ifdef, or #ifndef above it. Add the opening directive or remove this #elif." };
    }
    if (/^unterminated conditional directive in /.test(message)) {
        return { message: "A preprocessor #if, #ifdef, or #ifndef is missing its closing #endif." };
    }
    const macroArgumentCount = /^macro ([A-Za-z_]\w*) expects (\d+)(\+?) argument\(s\), got (\d+)$/.exec(message);
    if (macroArgumentCount) {
        return { message: `The macro ${macroArgumentCount[1]} was given ${macroArgumentCount[4]} argument${macroArgumentCount[4] === "1" ? "" : "s"}, but its #define requires ${macroArgumentCount[3] ? "at least " : ""}${macroArgumentCount[2]}. Check the commas inside its parentheses.` };
    }
    if (/^unterminated macro invocation$/.test(message)) {
        return { message: "This macro call is missing a closing parenthesis ')'." };
    }
    const missingMacroName = /^expected macro name after #(define|undef|ifdef|ifndef)$/.exec(message);
    if (missingMacroName) {
        const example = missingMacroName[1] === "define"
            ? "#define SIZE 10"
            : `#${missingMacroName[1]} FEATURE_NAME`;
        return { message: `#${missingMacroName[1]} needs a macro name after it, such as ${example}.` };
    }
    if (/^duplicate #else in conditional group$/.test(message)) {
        return { message: "This #if group already has an #else. Keep only one #else, or start a separate #if group." };
    }
    if (/^#elif cannot appear after #else$/.test(message)) {
        return { message: "#elif must come before #else in the same conditional group. Move this branch before #else or start a new #if." };
    }
    if (/^division by zero in #if expression$/.test(message)) {
        return { message: "This #if expression divides by zero. Change the compile-time condition so its divisor is not zero." };
    }
    if (/^invalid token in #if expression$/.test(message)) {
        return { message: "This #if condition contains a symbol that is not valid in a C preprocessor expression. Remove it or replace it with an integer expression." };
    }
    if (/^increment and decrement are not valid in #if expressions$/.test(message)) {
        return { message: "A #if condition cannot use ++ or -- because it only tests a compile-time integer expression; it cannot change a variable. Write the value or comparison you want to test." };
    }
    if (/^unexpected trailing tokens in #if expression$/.test(message)) {
        return { message: "C finished reading the #if condition but found extra text after it. Remove the extra text or add the missing operator between the expressions." };
    }
    if (/^expected primary expression in #if$/.test(message)) {
        return { message: "An operator in this #if condition is missing an operand. Add an integer, character constant, macro name, or parenthesized expression." };
    }
    if (/^expected \) in #if expression$/.test(message)) {
        return { message: "This #if condition is missing a closing parenthesis ')'." };
    }
    if (/^expected : in conditional expression$/.test(message)) {
        return { message: "The ?: expression in this #if condition is missing the colon between its two possible results." };
    }
    if (/^integer overflow in #if expression$/.test(message)) {
        return { message: "This compile-time integer calculation is outside the range supported in a #if condition. Use smaller values or rewrite the calculation." };
    }
    if (/^invalid shift count in #if expression$/.test(message)) {
        return { message: "The shift amount in this #if condition must be between 0 and 63. Use a smaller non-negative amount." };
    }
    if (/^(?:empty|unterminated) character constant in #if expression$/.test(message)) {
        return { message: "This #if condition contains an incomplete character constant. Put a character between matching single quotes, such as 'A'." };
    }
    if (/^(?:unterminated|unsupported|invalid|expected ).*(?:escape sequence|hexadecimal digits).*#if expression$/.test(message)) {
        return { message: "This character constant in the #if condition contains an invalid escape sequence. Check the backslash and the digits or escape letter after it." };
    }
    if (/^unsupported preprocessing directive on line \d+ in .+$/.test(message)) {
        return { message: "C does not recognize this preprocessor directive. Check its spelling or remove the line." };
    }
    if (/^duplicate macro parameter name$/.test(message)) {
        return { message: "A function-like macro cannot use the same parameter name twice. Rename or remove the duplicate parameter." };
    }
    if (/^# in macro replacement must be followed by a parameter name$/.test(message)) {
        return { message: "In a function-like macro, # can stringify only one of that macro's parameter names. Put a parameter after # or remove #." };
    }
    if (/^expected declaration specifiers$/.test(message)) {
        if (/\([^)]*,\s*\)/.test(text)) {
            return { message: "C does not allow a trailing comma at the end of a function parameter list. Remove the comma before )." };
        }
        if (/^}/.test(textAtDiagnostic) || /^\s*}/.test(text)) {
            return { message: "There is an extra closing brace '}' here, or an earlier block was already closed." };
        }
        const attemptedType = /^([A-Za-z_]\w*)/.exec(textAtDiagnostic)?.[1];
        if (attemptedType
            && declarationLooksLikeUnknownType(text, col, attemptedType)) {
            return {
                message: unknownTypeNameDiagnostic(lines, safeLine, col, attemptedType),
            };
        }
        return { message: "C expected a declaration here, such as a variable or function declaration. Check for a missing type, an extra brace, or a statement placed outside a function." };
    }
    if (/^object type cannot be void or function$/.test(message)) {
        return { message: "A variable needs an object type that can store a value." };
    }
    if (/^array bound must be non-negative$/.test(message)) {
        return { message: "Array sizes cannot be negative. Use a positive whole number inside the brackets." };
    }
    if (/^(?:object type must be complete|object definition has incomplete type .+)$/.test(message) && /\[\s*0\s*\]/.test(text)) {
        return { message: "Array sizes must be positive. Use at least 1 inside the brackets." };
    }
    if (/^string literal is too long for the destination array$/.test(message)) {
        return { message: "This string is too long for the char array. Leave room for every character plus the final '\\0' byte." };
    }
    if (/^too many initializer elements$/.test(message)) {
        return { message: "This initializer contains more values than the array, struct, or union can hold. Remove the extra value or increase the array size." };
    }
    if (/^initializer list cannot be empty in C11$/.test(message)) {
        return { message: "C11 does not allow an empty { } initializer. Supply at least one value, or omit the initializer to leave a local object uninitialized." };
    }
    if (/^array designator index must be non-negative$/.test(message)) {
        return { message: "An initializer's [index] cannot be negative. Use an index from 0 through the array's last element." };
    }
    if (/^array designator is outside the bounds of the array$/.test(message)) {
        return { message: "This initializer's [index] is outside the declared array. Increase the array size or use a smaller index." };
    }
    if (/^array designator requires an array type$/.test(message)) {
        return { message: "[index] selects an array element, but this initializer is not for an array. Use .member for a struct or union member." };
    }
    if (/^scalar initializer list must contain a single un-designated initializer$/.test(message)) {
        return { message: "A single-value variable can have only one value in its initializer. Remove the extra values or declare an array or struct instead." };
    }
    if (/^left shift count is negative or too large$/.test(message)) {
        return { message: "The shift amount must be between 0 and one less than the number of bits in the left value." };
    }
    if (/^shift count is negative or too large in constant expression$/.test(message)) {
        return { message: "This compile-time shift amount is negative or at least as large as the left operand's bit width. Use a smaller non-negative amount." };
    }
    if (/^signed integer overflow$/.test(message)) {
        return { message: "This calculation is outside the range of its signed integer type. Signed integer overflow is undefined in C." };
    }
    if (/^floating to integer conversion is outside the range of the destination type$/.test(message)) {
        return { message: "This floating-point value is outside the range of the destination integer type." };
    }
    const mismatchedArrayString = /^cannot initialize (.+) with (an? .+ string literal); use a brace-enclosed list or change the array element type to (.+)$/.exec(message);
    if (mismatchedArrayString) {
        const literal = mismatchedArrayString[2].replace(/^an? /, "");
        if (literal === "ordinary string literal") {
            return {
                message: "This string literal contains char elements, but this array's elements have a different type. Use a brace-enclosed list, or change the array's element type to char.",
            };
        }
        return {
            message: `This ${literal} has an element type that does not match the array. Use a brace-enclosed list, or change the array's element type to ${mismatchedArrayString[3]}.`,
        };
    }
    if (/^an array cannot be initialized by copying another array; use a brace-enclosed list instead$/.test(message)) {
        return {
            message: "C cannot initialize one array by copying another array. Initialize its elements with a brace-enclosed list instead.",
        };
    }
    const arrayConversion = /^cannot convert (.+) to (.+); an array of this type must use (.+)$/.exec(message);
    if (arrayConversion) {
        return {
            message: `This value has type ${arrayConversion[1]}, so it cannot initialize ${arrayConversion[2]}. Use ${arrayConversion[3]}.`,
        };
    }
    const cannotConvert = /^cannot convert (.+) to (.+)$/.exec(message);
    if (cannotConvert) {
        const sourceType = cannotConvert[1];
        const targetType = cannotConvert[2];
        if (sourceType.endsWith("*") && targetType.endsWith("*")) {
            return { message: `This expression has type ${sourceType}, but this location expects ${targetType}. The pointers refer to different element types; make the pointed-to types agree.` };
        }
        if (sourceType.endsWith("*") && !targetType.endsWith("*")) {
            const addressOfIsHighlighted = /^\(*\s*&/.test(textAtDiagnostic) || /&\s*$/.test(text.slice(0, col));
            if (addressOfIsHighlighted) {
                const article = /^[aeiou]/i.test(targetType) ? "an" : "a";
                return { message: `& produces an address of type ${sourceType}, but this location expects ${article} ${targetType} value. Remove & if you intended to use the object's value.` };
            }
            if (sourceType === "char*" && targetType === "char" && /^"/.test(textAtDiagnostic)) {
                return {
                    message: "This expression is a string (a sequence of characters), but this location is a char variable and holds one character. Use single quotes for one character, such as char letter = 'h'; or change the declaration to a char array, such as char text[] = \"hi\";.",
                };
            }
            const article = /^[aeiou]/i.test(targetType) ? "an" : "a";
            return {
                message: `This expression has pointer type ${sourceType}, so it represents an address, but this location expects ${article} ${targetType} value. If you need the value it points to, use * on the pointer expression. If you need the address, change the destination to a compatible pointer type.`,
            };
        }
        return { message: `This expression has type ${cannotConvert[1]}, but this location requires ${cannotConvert[2]}. Supply a value of the required type, or change the destination's declaration if its type is wrong.` };
    }
    const undefinedFunction = /^call to undefined function ([A-Za-z_]\w*)$/.exec(message);
    if (undefinedFunction) {
        return { message: `cBoxes knows the name ${undefinedFunction[1]}, but this function is not available to run here. Check the include and function name.` };
    }
    const printfType = /^(printf|fprintf|sprintf|snprintf) (%[^ ]+) requires an argument of type (.+)$/.exec(message);
    if (printfType) {
        return { message: `${printfType[1]}'s ${printfType[2]} conversion does not match the supplied value. It requires type ${printfType[3]}; change the conversion or pass a value of that type.` };
    }
    const printfMissing = /^(printf|fprintf|sprintf|snprintf) is missing an argument for (%\S+)$/.exec(message);
    if (printfMissing) {
        return { message: `${printfMissing[1]}'s format contains ${printfMissing[2]}, but there is no corresponding value argument. Add the missing argument or remove that conversion.` };
    }
    const scanfType = /^(scanf|fscanf|sscanf) argument must have type (.+)$/.exec(message);
    if (scanfType) {
        return { message: `${scanfType[1]}'s conversion does not match the destination pointer. It needs ${scanfType[2]}; change the conversion or pass the address of a matching variable.` };
    }
    const failedAssertion = /^assertion failed: (.+)$/.exec(message);
    if (failedAssertion) {
        return { message: `The assertion ${failedAssertion[1]} was false. Inspect the values used by this condition or correct the condition if it states the wrong expectation.` };
    }
    if (/^read of uninitialized automatic object/.test(message)) {
        if (/\bscanf\s*\([^,]+,\s*[A-Za-z_]\w*\s*\)/.test(text)) {
            return { message: "scanf needs the address where it should store the input. Put & before this numeric variable, as in scanf(\"%d\", &x)." };
        }
        if (/\b[A-Za-z_]\w*\s*==/.test(text)) {
            return { message: "This reads a variable before it has a value. If you meant to assign a value, use = instead of ==." };
        }
        if (/^[A-Za-z_]\w*\s*\[[^\]]+\]/.test(textAtDiagnostic)) {
            return { message: "This array element is read before a value has been stored in it. Initialize the array or assign this element first." };
        }
        if (/^[A-Za-z_]\w*\.[A-Za-z_]\w*/.test(textAtDiagnostic)) {
            return { message: "This struct or union member is read before a value has been stored in it. Initialize the object or assign this member first." };
        }
        const derefName = dereferencedAssignmentName(text);
        if (derefName && declaredAsPointerBefore(lines, safeLine, derefName)) {
            return { message: `The pointer ${derefName} does not point anywhere yet. Set it to an address, like &a, before using *${derefName}.` };
        }
        if (derefName) {
            return { message: "The * operator only works on pointers. Make sure this variable has a pointer type before writing through *." };
        }
        const dereferencedRead = /\*\s*([A-Za-z_]\w*)/.exec(text)?.[1] ?? null;
        if (dereferencedRead && declaredAsPointerBefore(lines, safeLine, dereferencedRead)) {
            return { message: `The pointer ${dereferencedRead} has not been given an address yet. Point it at an object, such as ${dereferencedRead} = &x, before dereferencing it.` };
        }
        if (/^\s*(?:int|double|float|char|short|long|bool)\s+\*\s*[A-Za-z_]\w*\s*=\s*[A-Za-z_]\w*\s*;/.test(text)) {
            return { message: "A pointer stores an address. Use & before the variable name to store its address, like int *p = &a;." };
        }
        return { message: "This reads a variable before it has been given a value. Assign it a value first." };
    }
    if (/^invalid operands to binary/.test(message)) {
        return { message: "This operator does not work with the types on its left and right sides." };
    }
    if (/^attempt to modify (?:a const-qualified object or subobject|an object defined with a const-qualified type)$/.test(message)) {
        return { message: "This operation would change an object declared const. Const objects and their const members are read-only after initialization." };
    }
    if (/^array lvalue refers to an object whose lifetime has ended$/.test(message)) {
        return { message: "This array belonged to an object or block that no longer exists. Do not keep or use a pointer to a local array after its lifetime ends." };
    }
    if (/^array lvalue does not designate a valid subarray$/.test(message)) {
        return { message: "This expression no longer refers to a valid array region. Check the pointer conversion, member access, and array bounds that produced it." };
    }
    if (/indeterminate/.test(message)) {
        return { message: "This operation uses a value that was never initialized or that became invalid. Initialize every byte or pointer field before reading or passing the object." };
    }
    if (/invalid object representation|object representation has the wrong size/.test(message)) {
        return { message: "These stored bytes do not represent a valid value of this type. This can result from reading uninitialized memory, copying the wrong number of bytes, or reinterpreting memory through an incompatible type." };
    }
    if (/effective type/.test(message)) {
        return { message: "This memory is being accessed through an incompatible pointer type. A cast changes the pointer's declared type but does not make an unrelated object safe to read through it." };
    }
    if (/restrict-qualified.*overlap|restrict-qualified accesses overlap/.test(message)) {
        return { message: "These restrict-qualified accesses overlap even though the function or declaration promises they refer to separate regions. Use non-overlapping objects." };
    }
    const unsupportedRange = /^(.+) is out of supported range$/.exec(message);
    if (unsupportedRange) {
        return { message: `${unsupportedRange[1]} is too large or otherwise outside the range cBoxes can represent safely. Check for a negative value converted to an unsigned size, arithmetic overflow, or an unexpectedly large input.` };
    }
    const unsupportedLibraryFunction = /^unsupported (?:host )?library function ([A-Za-z_]\w*)$/.exec(message);
    if (unsupportedLibraryFunction) {
        return { message: `${unsupportedLibraryFunction[1]} is not available in cBoxes. Check the function name and header, or use a supported C11 library alternative.` };
    }
    const unsupportedMathFunction = /^unsupported (?:unary|binary|ternary)?\s*math function ([A-Za-z_]\w*)$/.exec(message);
    if (unsupportedMathFunction) {
        return { message: `${unsupportedMathFunction[1]} is not implemented by cBoxes. Check the spelling and <math.h>, or use an available C11 math function.` };
    }
    if (/^(?:unsupported call target|function pointer does not name a supported function)$/.test(message)) {
        return { message: "This function pointer does not identify a function that cBoxes can call. Initialize it from a compatible function name and do not manufacture it with an integer or unrelated cast." };
    }
    if (/\blvalue\b/.test(message)) {
        return { message: "This argument must be a named, stored object that C can update—not a temporary expression or computed value." };
    }
    return { message };
}
function rustDiagnostic(raw, source) {
    const rawLine = Math.max(0, Math.floor(Number(raw.line ?? 0)));
    const rawCol = Math.max(0, Math.floor(Number(raw.column ?? 0)));
    const lines = sourceLines(source);
    const compact = compactDiagnosticMessage(raw.message);
    const friendly = friendlyDiagnosticFor(compact, source, rawLine, rawCol);
    const line = clampLine(friendly.line ?? rawLine, lines);
    const col = Math.max(0, Math.min(lineEndColumn(lines, line), Math.floor(friendly.col ?? rawCol)));
    const hasFriendlyRange = friendly.line !== undefined
        || friendly.col !== undefined
        || friendly.endCol !== undefined;
    let endLine = line;
    let endCol = Math.max(col + 1, Math.floor(friendly.endCol ?? col + 1));
    if (!hasFriendlyRange && raw.endLine != null && raw.endColumn != null) {
        endLine = clampLine(Math.max(0, Math.floor(Number(raw.endLine))), lines);
        endCol = Math.max(0, Math.min(lineEndColumn(lines, endLine), Math.floor(Number(raw.endColumn))));
        if (endLine < line || (endLine === line && endCol <= col)) {
            endLine = line;
            endCol = col + 1;
        }
    }
    const range = {
        startLine: line,
        startCol: col,
        endLine,
        endCol,
    };
    const rawAnnotations = (raw.annotations || [])
        .filter((annotation) => annotation.id && annotation.line != null && annotation.column != null)
        .map((annotation) => {
        const startLine = Math.max(0, Math.floor(Number(annotation.line)));
        const startCol = Math.max(0, Math.floor(Number(annotation.column)));
        const endLine = Math.max(startLine, Math.floor(Number(annotation.endLine ?? startLine)));
        const endCol = Math.max(endLine === startLine ? startCol + 1 : 0, Math.floor(Number(annotation.endColumn ?? startCol + 1)));
        return {
            id: String(annotation.id),
            file: annotation.file || raw.file || undefined,
            range: { startLine, startCol, endLine, endCol },
        };
    });
    const rawDestinationRange = rawAnnotations.find((annotation) => annotation.id === "destination")?.range;
    const destinationRange = rawDestinationRange
        || (/\b(?:This|this) location\b/.test(friendly.message)
            ? destinationRangeBeforeExpression(lines, range)
            : null);
    const visibleMessage = destinationRange
        ? friendly.message
        : friendly.message.replace(/\bthis location\b/gi, "the destination");
    return {
        kind: raw.kind,
        message: visibleMessage,
        file: raw.file || undefined,
        range,
        annotations: rawAnnotations.length
            ? rawAnnotations
            : destinationRange
                ? [{ id: "destination", file: raw.file || undefined, range: destinationRange }]
                : undefined,
        messageParts: linkedDiagnosticMessageParts(visibleMessage, !!destinationRange),
    };
}
function compileDiagnostic(message) {
    return {
        kind: "compile",
        diagnostic: {
            kind: "compile",
            message,
            range: {
                startLine: 0,
                startCol: 0,
                endLine: 0,
                endCol: 1,
            },
        },
    };
}
function crashDiagnostic(error) {
    if (error instanceof WasiProcExit) {
        return compileDiagnostic("The interpreter stopped while checking this program.");
    }
    const message = error instanceof Error && error.message
        ? `The interpreter stopped while checking this program: ${error.message}`
        : "The interpreter stopped while checking this program.";
    return compileDiagnostic(message);
}
function compileExpressionDiagnostic(message) {
    return {
        kind: "compile",
        diagnostic: {
            kind: "compile",
            message,
            range: {
                startLine: 0,
                startCol: 0,
                endLine: 0,
                endCol: 1,
            },
        },
    };
}
function crashExpressionDiagnostic(error) {
    if (error instanceof WasiProcExit) {
        return compileExpressionDiagnostic("The interpreter stopped while checking this expression.");
    }
    const message = error instanceof Error && error.message
        ? `The interpreter stopped while checking this expression: ${error.message}`
        : "The interpreter stopped while checking this expression.";
    return compileExpressionDiagnostic(message);
}
function compactDiagnosticMessage(message) {
    const firstLine = String(message || "").split(/\r?\n/, 1)[0] || "The program did not compile.";
    return firstLine
        .replace(/^error:\s*/i, "")
        .replace(/^undefined behavior:\s*/i, "");
}
function normalizeState(rawState) {
    if (!Array.isArray(rawState))
        return [];
    return rawState.map((item) => {
        const box = item;
        return {
            name: String(box.name ?? ""),
            type: String(box.type ?? "int"),
            value: String(box.value ?? ""),
            displayValue: String(box.displayValue ?? box.value ?? ""),
            exactValue: String(box.exactValue ?? box.displayValue ?? box.value ?? ""),
            address: box.address == null ? null : String(box.address),
            arrayRoot: box.arrayRoot == null ? null : String(box.arrayRoot),
            arrayShape: Array.isArray(box.arrayShape)
                ? box.arrayShape.map((value) => Number(value)).filter(Number.isFinite)
                : null,
            arrayIndices: Array.isArray(box.arrayIndices)
                ? box.arrayIndices.map((value) => Number(value)).filter(Number.isFinite)
                : null,
            aggregateRoot: box.aggregateRoot == null ? null : String(box.aggregateRoot),
            aggregatePath: Array.isArray(box.aggregatePath)
                ? box.aggregatePath.map((value) => String(value))
                : null,
            aggregateKind: box.aggregateKind === "struct" || box.aggregateKind === "union"
                ? box.aggregateKind
                : null,
            aliases: Array.isArray(box.aliases)
                ? box.aliases.map((value) => String(value))
                : [],
            typeInfo: normalizeTypeInfo(box.typeInfo),
        };
    });
}
function normalizeTypeInfo(raw) {
    const info = raw && typeof raw === "object" ? raw : {};
    const normalizeShape = (value) => Array.isArray(value)
        ? value
            .map((item) => Math.floor(Number(item)))
            .filter((item) => Number.isFinite(item) && item >= 0)
        : [];
    const nullableNumber = (value) => {
        const numeric = Number(value);
        return value == null || !Number.isFinite(numeric) ? null : numeric;
    };
    const kinds = new Set([
        "void", "integer", "floating", "complex", "pointer", "array",
        "aggregate", "function", "va-list", "unknown",
    ]);
    const kind = String(info.kind ?? "unknown");
    const normalizeHelpNode = (value, depth = 0) => {
        if (!value || typeof value !== "object" || depth > 24)
            return null;
        const node = value;
        const nodeKind = String(node.kind ?? "type");
        const allowedKinds = new Set(["pointer", "array", "function", "type"]);
        return {
            kind: allowedKinds.has(nodeKind)
                ? nodeKind
                : "type",
            label: String(node.label ?? ""),
            typeName: node.typeName == null ? null : String(node.typeName),
            children: Array.isArray(node.children)
                ? node.children.flatMap((rawChild) => {
                    if (!rawChild || typeof rawChild !== "object")
                        return [];
                    const child = rawChild;
                    const childNode = normalizeHelpNode(child.node, depth + 1);
                    return childNode
                        ? [{ relation: String(child.relation ?? ""), node: childNode }]
                        : [];
                })
                : [],
        };
    };
    return {
        kind: kinds.has(kind) ? kind : "unknown",
        help: info.help == null ? null : String(info.help),
        helpTypeNames: Array.isArray(info.helpTypeNames)
            ? info.helpTypeNames.map((name) => String(name))
            : [],
        helpTree: normalizeHelpNode(info.helpTree),
        pointerDepth: Math.max(0, Math.floor(Number(info.pointerDepth ?? 0))),
        arrayShape: normalizeShape(info.arrayShape),
        pointeeArrayShape: normalizeShape(info.pointeeArrayShape),
        size: nullableNumber(info.size),
        align: nullableNumber(info.align),
    };
}
function normalizeTrace(rawTrace) {
    if (!Array.isArray(rawTrace))
        return [];
    return rawTrace.map((item) => {
        const event = item;
        return {
            kind: String(event.kind ?? ""),
            file: String(event.file ?? "program.c"),
            startLine: Math.max(0, Math.floor(Number(event.startLine ?? 0))),
            endLine: Math.max(0, Math.floor(Number(event.endLine ?? event.startLine ?? 0))),
            state: normalizeState(event.state),
            skippedRange: normalizeProgramSourceRange(event.skippedRange),
        };
    });
}
function normalizeProgramSourceRange(rawRange) {
    if (!rawRange || typeof rawRange !== "object")
        return null;
    const range = rawRange;
    return {
        file: String(range.file ?? "program.c"),
        startLine: Math.max(0, Math.floor(Number(range.startLine ?? 0))),
        startColumn: Math.max(0, Math.floor(Number(range.startColumn ?? 0))),
        endLine: Math.max(0, Math.floor(Number(range.endLine ?? range.startLine ?? 0))),
        endColumn: Math.max(0, Math.floor(Number(range.endColumn ?? 0))),
    };
}
function normalizeSourceLocation(rawLocation) {
    if (!rawLocation || typeof rawLocation !== "object")
        return null;
    const location = rawLocation;
    const line = Number(location.line);
    if (!Number.isFinite(line))
        return null;
    return {
        file: String(location.file ?? "program.c"),
        line: Math.max(0, Math.floor(line)),
    };
}
function normalizeBlocked(rawBlocked) {
    if (!rawBlocked || typeof rawBlocked !== "object")
        return null;
    const blocked = rawBlocked;
    const startLine = Number(blocked.startLine);
    const endLine = Number(blocked.endLine ?? blocked.startLine);
    if (!Number.isFinite(startLine) || !Number.isFinite(endLine))
        return null;
    return {
        file: String(blocked.file ?? "program.c"),
        startLine: Math.max(0, Math.floor(startLine)),
        endLine: Math.max(0, Math.floor(endLine)),
        function: String(blocked.function ?? "input"),
        state: normalizeState(blocked.state),
    };
}
function normalizeExecutionLimit(rawLimit) {
    if (!rawLimit || typeof rawLimit !== "object")
        return null;
    const limit = rawLimit;
    const startLine = Number(limit.startLine);
    const endLine = Number(limit.endLine ?? limit.startLine);
    const tracePosition = Number(limit.tracePosition);
    if (!Number.isFinite(startLine) ||
        !Number.isFinite(endLine) ||
        !Number.isFinite(tracePosition)) {
        return null;
    }
    return {
        file: String(limit.file ?? "program.c"),
        startLine: Math.max(0, Math.floor(startLine)),
        endLine: Math.max(0, Math.floor(endLine)),
        tracePosition: Math.max(0, Math.floor(tracePosition)),
    };
}
const textEncoder = new TextEncoder();
const textDecoder = new TextDecoder();
function invokeInterpreter(inputs, invoke) {
    const interpreter = interpreterExports();
    const allocations = [];
    let outputPointer = null;
    let outputLength = 0;
    try {
        const pointers = inputs.map((input) => {
            const pointer = interpreter.cboxes_alloc(input.length);
            new Uint8Array(interpreter.memory.buffer).set(input, pointer);
            allocations.push({ pointer, length: input.length });
            return pointer;
        });
        outputPointer = invoke(interpreter, pointers);
        outputLength = interpreter.cboxes_last_result_len();
        const json = textDecoder.decode(new Uint8Array(interpreter.memory.buffer, outputPointer, outputLength));
        return JSON.parse(json);
    }
    finally {
        try {
            for (const allocation of allocations) {
                interpreter.cboxes_free(allocation.pointer, allocation.length);
            }
            if (outputPointer != null) {
                interpreter.cboxes_free(outputPointer, outputLength);
            }
        }
        catch {
            exportsCache = null;
        }
    }
}
function normalizeProgramResult(parsed, diagnosticSource) {
    const implicit = {
        implicitMainApplied: parsed.implicitMainApplied,
        implicitMainNotice: parsed.implicitMainNotice,
    };
    if (!parsed.ok) {
        return {
            kind: parsed.kind,
            diagnostic: rustDiagnostic(parsed, diagnosticSource),
            ...implicit,
        };
    }
    return {
        kind: "ok",
        state: normalizeState(parsed.state),
        trace: normalizeTrace(parsed.trace),
        mainClose: normalizeSourceLocation(parsed.mainClose),
        blocked: normalizeBlocked(parsed.blocked),
        executionLimit: normalizeExecutionLimit(parsed.executionLimit),
        stdout: String(parsed.stdout ?? ""),
        stderr: String(parsed.stderr ?? ""),
        exitStatus: Number(parsed.exitStatus ?? 0),
        ...implicit,
    };
}
function normalizeExpressionResult(parsed, diagnosticSource) {
    if (!parsed.ok) {
        return {
            kind: parsed.kind,
            diagnostic: rustDiagnostic(parsed, diagnosticSource),
        };
    }
    const result = parsed.result;
    return {
        kind: "ok",
        result: {
            kind: String(result.kind ?? "rvalue"),
            type: String(result.type ?? "int"),
            value: String(result.value ?? ""),
            displayValue: String(result.displayValue ?? result.value ?? ""),
            exactValue: String(result.exactValue ?? result.displayValue ?? result.value ?? ""),
            address: result.address == null ? "" : String(result.address),
            name: result.name == null ? undefined : String(result.name),
            valueLiteral: result.valueLiteral?.kind === "integer" ||
                result.valueLiteral?.kind === "floating"
                ? {
                    kind: result.valueLiteral.kind,
                    hasSuffix: result.valueLiteral.hasSuffix === true,
                }
                : null,
            typeInfo: normalizeTypeInfo(result.typeInfo),
        },
    };
}
export function runCProgram(source, addressBase = syntheticAddressBase, stdin = "") {
    try {
        const sourceInput = textEncoder.encode(source);
        const stdinInput = textEncoder.encode(stdin);
        const parsed = invokeInterpreter([sourceInput, stdinInput], (interpreter, [sourcePointer, stdinPointer]) => interpreter.cboxes_run_source(sourcePointer, sourceInput.length, stdinPointer, stdinInput.length, addressBase));
        return normalizeProgramResult(parsed, source);
    }
    catch (error) {
        exportsCache = null;
        return crashDiagnostic(error);
    }
}
export function runCFiles(files, addressBase = syntheticAddressBase, stdin = "", implicitMain = true, executionBudget = DEFAULT_EXECUTION_BUDGET) {
    const bundle = encodeSourceFiles(files);
    const stdinInput = textEncoder.encode(stdin);
    const budget = normalizeExecutionBudget(executionBudget);
    try {
        const parsed = invokeInterpreter([bundle, stdinInput], (interpreter, [bundlePointer, stdinPointer]) => interpreter.cboxes_run_files(bundlePointer, bundle.length, stdinPointer, stdinInput.length, addressBase, implicitMain ? 1 : 0, budget.stepLimit, budget.followingTraceLimit));
        const diagnosticSource = !parsed.ok
            ? files.find((file) => file.path === parsed.file)?.source ??
                files[0]?.source ??
                ""
            : files[0]?.source ?? "";
        return normalizeProgramResult(parsed, diagnosticSource);
    }
    catch (error) {
        exportsCache = null;
        return crashDiagnostic(error);
    }
}
export function evaluateCExpression(source, eventIndex, expression, addressBase = syntheticAddressBase, stdin = "") {
    const sourceInput = textEncoder.encode(source);
    const expressionInput = textEncoder.encode(expression);
    const stdinInput = textEncoder.encode(stdin);
    try {
        const parsed = invokeInterpreter([sourceInput, expressionInput, stdinInput], (interpreter, [sourcePointer, expressionPointer, stdinPointer]) => interpreter.cboxes_eval_expression(sourcePointer, sourceInput.length, expressionPointer, expressionInput.length, Math.max(0, Math.floor(eventIndex)), stdinPointer, stdinInput.length, addressBase));
        return normalizeExpressionResult(parsed, expression);
    }
    catch (error) {
        exportsCache = null;
        return crashExpressionDiagnostic(error);
    }
}
export function evaluateCExpressionFiles(files, eventIndex, expression, addressBase = syntheticAddressBase, stdin = "", implicitMain = true, executionBudget = DEFAULT_EXECUTION_BUDGET) {
    const bundle = encodeSourceFiles(files);
    const expressionInput = textEncoder.encode(expression);
    const stdinInput = textEncoder.encode(stdin);
    const budget = normalizeExecutionBudget(executionBudget);
    try {
        const parsed = invokeInterpreter([bundle, expressionInput, stdinInput], (interpreter, [bundlePointer, expressionPointer, stdinPointer]) => interpreter.cboxes_eval_expression_files(bundlePointer, bundle.length, expressionPointer, expressionInput.length, Math.max(0, Math.floor(eventIndex)), stdinPointer, stdinInput.length, addressBase, implicitMain ? 1 : 0, budget.stepLimit));
        const diagnosticSource = !parsed.ok && parsed.file !== "<expression>"
            ? files.find((file) => file.path === parsed.file)?.source ??
                files[0]?.source ??
                ""
            : expression;
        return normalizeExpressionResult(parsed, diagnosticSource);
    }
    catch (error) {
        exportsCache = null;
        return crashExpressionDiagnostic(error);
    }
}
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
const typeInfoCache = new Map();
function inspectCType(type) {
    const normalized = type.trim();
    if (!normalized)
        return null;
    const cached = typeInfoCache.get(normalized);
    if (cached)
        return cached;
    const evaluated = evaluateCExpression("0;", 0, `(${normalized})0`);
    if (evaluated.kind !== "ok")
        return null;
    typeInfoCache.set(normalized, evaluated.result.typeInfo);
    return evaluated.result.typeInfo;
}
export function resolveCBoxAliases(boxes) {
    const resolved = boxes.map((box) => ({ ...box, aliases: [] }));
    const byAddress = new Map(resolved
        .filter((box) => String(box.address ?? "").trim())
        .map((box) => [String(box.address).trim(), box]));
    for (const pointer of resolved) {
        const pointerDepth = inspectCType(pointer.type)?.pointerDepth ?? 0;
        if (!pointer.name || pointerDepth === 0)
            continue;
        let address = String(pointer.value ?? "").trim();
        for (let level = 1; level <= pointerDepth; level += 1) {
            const target = byAddress.get(address);
            if (!target)
                break;
            target.aliases.push(`${"*".repeat(level)}${pointer.name}`);
            address = String(target.value ?? "").trim();
        }
    }
    return resolved;
}
const workspaceAddressSlots = new Map();
function loadWorkspaceAddressSlots(type, count) {
    const cached = workspaceAddressSlots.get(type) ?? [];
    if (cached.length >= count)
        return cached;
    const source = Array.from({ length: count }, (_, index) => `${type} __cboxes_address_slot_${index};`).join("\n");
    const run = runCProgram(source);
    const slots = run.kind === "ok"
        ? run.state.filter((box) => box.name.startsWith("__cboxes_address_slot_"))
        : [];
    workspaceAddressSlots.set(type, slots);
    return slots;
}
export function allocateCWorkspaceObject(boxes, requestedType) {
    const enteredType = requestedType.trim();
    const enteredTypeInfo = enteredType ? inspectCType(enteredType) : null;
    const assumedType = enteredTypeInfo ? enteredType : "int";
    const requestedTypeInfo = enteredTypeInfo ?? inspectCType("int");
    if (!requestedTypeInfo)
        return null;
    const occupied = boxes.flatMap((box) => {
        const start = Number(String(box.address ?? "").trim());
        if (!Number.isFinite(start))
            return [];
        const size = Math.max(1, inspectCType(String(box.type || ""))?.size ?? box.typeInfo?.size ?? 1);
        return [{ start, end: start + size }];
    });
    const allocationFrontier = occupied.reduce((frontier, range) => Math.max(frontier, range.end), Number.NEGATIVE_INFINITY);
    let slotCount = Math.max(32, boxes.length * 2 + 8);
    for (let attempt = 0; attempt < 5; attempt += 1) {
        const slots = loadWorkspaceAddressSlots(assumedType, slotCount);
        for (const slot of slots) {
            const address = String(slot.address ?? "").trim();
            const start = Number(address);
            if (!address || !Number.isFinite(start))
                continue;
            const typeInfo = slot.typeInfo ?? requestedTypeInfo;
            const size = Math.max(1, typeInfo.size ?? 1);
            const end = start + size;
            if (start >= allocationFrontier &&
                occupied.every((range) => end <= range.start || start >= range.end)) {
                return { address, typeInfo, assumedType };
            }
        }
        slotCount *= 2;
    }
    return null;
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
        // Synthetic addresses are opaque identifiers issued by the interpreter.
        // Re-evaluating one in an isolated expression would lose the allocation it
        // refers to, so pointer answers compare those Rust-produced identifiers.
        const ok = actualRaw === expectedRaw;
        return { ok, normalized: ok ? expectedRaw : "" };
    }
    const entered = evaluateCExpression("0;", 0, actualRaw);
    if (entered.kind !== "ok" || !entered.result.valueLiteral) {
        return { ok: false, normalized: "" };
    }
    if ((targetKind === "floating" && entered.result.typeInfo.kind !== "floating") ||
        (targetKind === "integer" && entered.result.typeInfo.kind !== "integer")) {
        return { ok: false, normalized: "" };
    }
    const cast = (raw) => evaluateCExpression("0;", 0, `(${targetType})(${raw})`);
    const convertedActual = cast(actualRaw);
    const convertedExpected = cast(expectedRaw);
    if (convertedActual.kind !== "ok" || convertedExpected.kind !== "ok") {
        return { ok: false, normalized: "" };
    }
    const ok = convertedActual.result.value === convertedExpected.result.value &&
        convertedActual.result.exactValue === convertedExpected.result.exactValue;
    return {
        ok,
        normalized: ok ? convertedExpected.result.value : "",
    };
}
