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
function wasiImports(getExports) {
    const errnoNosys = 52;
    const ok = 0;
    return {
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
function lineLooksLikeForgottenComma(line) {
    return /^\s*(?:const\s+)?(?:unsigned\s+|signed\s+)?(?:int|double|float|char|short|long|bool)\s+\*?\s*[A-Za-z_]\w*\s+[A-Za-z_]\w*/.test(line);
}
function declaredAsPointerBefore(lines, beforeLine, name) {
    const escaped = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const declaration = new RegExp(`\\b(?:const\\s+)?(?:unsigned\\s+|signed\\s+)?(?:int|double|float|char|short|long|bool|void)\\s*\\*+\\s*${escaped}\\b`);
    for (let index = 0; index < Math.min(beforeLine, lines.length); index += 1) {
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
    const atEnd = line >= lines.length;
    if (/^expected Semicolon$/.test(message)) {
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
        if (/^\s*(?:int|double|float|char|short|long|void)\s+[A-Za-z_]\w*\s*\{/.test(text)) {
            return { message: "Function definitions need parentheses after the function name, like int f(void) { ... }." };
        }
        if (/^\s*(?:int|double|float|char|short|long|bool)\s+[A-Za-z_]\w*-[A-Za-z_]\w*/.test(text)) {
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
    if (/^expected LParen$/.test(message)) {
        const keyword = /\bwhile\b/.test(text) ? "while" : /\bif\b/.test(text) ? "if" : null;
        if (keyword) {
            return {
                message: `Put the ${keyword} condition in parentheses, like ${keyword} (condition) { ... }.`,
            };
        }
        return { message: "Add an opening parenthesis '(' here." };
    }
    if (/^expected RParen$/.test(message)) {
        if (/\b(if|while)\b/.test(text)) {
            return {
                message: "Add a closing parenthesis ')' after the condition, before the opening brace.",
            };
        }
        return { message: "Add a closing parenthesis ')' here." };
    }
    if (/^expected RBracket$/.test(message)) {
        return { message: "Add a closing bracket ']' here." };
    }
    if (/^expected LBrace$/.test(message)) {
        return { message: "Add an opening brace '{' to start this block." };
    }
    if (/^expected RBrace$/.test(message)) {
        return { message: "Add a closing brace '}' to end this block." };
    }
    if (/^expected identifier$/.test(message)) {
        if (/^\s*(?:const\s+)?(?:unsigned\s+|signed\s+)?(?:int|double|float|char|short|long|bool)\s+\d/.test(text)) {
            return { message: "Variable names cannot start with a number. Start the name with a letter or underscore." };
        }
        if (/,\s*;/.test(text)) {
            return {
                message: "After a comma in a declaration, write another variable name, or remove the comma.",
            };
        }
        return { message: "C expected a name here. Use a variable, function, member, or label name." };
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
        if (/^\s*else\b/.test(text)) {
            return { message: "else must come right after an if block. Check that the if and its braces come before this else." };
        }
        if (/=\s*;/.test(text)) {
            return { message: "Put a value or expression after the equals sign, before the semicolon." };
        }
        if (/[+\-*/%<>=!&|]\s*;/.test(text)) {
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
    const undeclared = /^use of undeclared identifier ([A-Za-z_]\w*)$/.exec(message);
    if (undeclared) {
        const name = undeclared[1] || "";
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
        return {
            message: `C does not know what ${name} is yet. Declare a variable named ${name} before using it, or check the spelling.`,
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
    if (/unterminated quoted literal/.test(message) || /^unterminated string literal$/.test(message)) {
        if (/^\s*#\s*include/.test(text)) {
            return { message: "This #include is missing the closing quote or > for the header name." };
        }
        return { message: "This string is missing its closing double quote (\")." };
    }
    if (/^empty character constant$/.test(message)) {
        return { message: "A character literal needs one character between the single quotes, like 'a'." };
    }
    if (/^unterminated character constant$/.test(message)) {
        return { message: "This character literal is missing its closing single quote (')." };
    }
    if (/^unterminated block comment/.test(message)) {
        return { message: "This block comment is missing its closing */." };
    }
    const unexpected = /^unexpected character (.+)$/.exec(message);
    if (unexpected) {
        return { message: `C does not use ${unexpected[1]} here. Remove it or replace it with the right C operator or punctuation.` };
    }
    if (/^operand of & must be an lvalue$/.test(message)) {
        if (/&\s*(?:\d|')/.test(text)) {
            return { message: "The & operator takes the address of a variable. A number or character literal does not have an address you can use here." };
        }
        return { message: "The & operator can only take the address of a variable, array element, or similar stored object." };
    }
    if (/^object type must be complete$/.test(message) && /\[\s*\]/.test(text)) {
        return { message: "C needs to know the array size here. Write a size inside the brackets, like int a[3];." };
    }
    if (/^initializer for static storage duration object is not a compile-time constant$/.test(message)) {
        return {
            message: "A global variable can only be initialized with a constant value. Move this assignment into main, or use a literal constant here.",
        };
    }
    if (/^left operand of assignment is not assignable$/.test(message)) {
        return { message: "The left side of = must be something you can store into, like a variable, *pointer, or array element." };
    }
    if (/^left operand of assignment is not a modifiable lvalue$/.test(message)) {
        if (/^\s*[A-Za-z_]\w*\s*=/.test(text)) {
            return { message: "This variable cannot be changed here. It may be const, an array name, or another value C does not allow on the left side of =." };
        }
        return { message: "The left side of = must be a variable or location that C allows you to change." };
    }
    if (/^division by zero$/.test(message)) {
        return { message: "This divides by zero, which C does not allow." };
    }
    if (/^remainder with a zero divisor$/.test(message)) {
        return { message: "This uses % with zero on the right side, which C does not allow." };
    }
    if (/^dereference of a null pointer$/.test(message)) {
        return { message: "This pointer is null (0), so it does not point to a variable you can use." };
    }
    if (/^pointer is not valid to dereference$/.test(message) && /\[[^\]]+\]/.test(text)) {
        return { message: "This array index is outside the array. For int a[3], the valid indexes are 0, 1, and 2." };
    }
    if (/^pointer arithmetic produced a pointer outside the bounds of the object$/.test(message) && /\[[^\]]+\]/.test(text)) {
        return { message: "This array index is outside the array bounds." };
    }
    if (/^unsupported operands for pointer arithmetic$/.test(message) && /\[[^\]]*\d+\.\d+[^\]]*\]/.test(text)) {
        return { message: "Array indexes must be whole-number integer expressions, not decimals." };
    }
    if (/^integer expression is not a null pointer constant$/.test(message)) {
        return { message: "A pointer can only be set to an address, like &a, or to 0/null. Use & if you meant the variable's address." };
    }
    if (/^equality comparison requires compatible pointer operand types$/.test(message)) {
        return { message: "These pointers point to different types, so C will not compare them directly. Make the pointer types match first." };
    }
    const noMember = /^(.+) has no member named ([A-Za-z_]\w*)$/.exec(message);
    if (noMember) {
        return { message: `The . operator is for structs and unions. A value of type ${noMember[1]} does not have a field named ${noMember[2]}.` };
    }
    if (/^operand of \* must have pointer type$/.test(message)) {
        if (/\*\*/.test(text)) {
            return { message: "C does not use ** for powers. Multiply explicitly, or use pow from <math.h> when you need exponentiation." };
        }
        return { message: "The * dereference operator only works on pointers." };
    }
    if (/^break statement is not within a loop$/.test(message)) {
        return { message: "break only works inside a loop or switch. Move it into a loop, or remove it." };
    }
    if (/^continue statement is not within a loop$/.test(message)) {
        return { message: "continue only works inside a loop. Move it into a loop, or remove it." };
    }
    const missingHeader = /^header file "([^"]+)" is not available$/.exec(message);
    if (missingHeader) {
        return { message: `The header ${missingHeader[1]} was not found. Add that file to the project, correct the include name, or use a supported standard header.` };
    }
    if (/^expected declaration specifiers$/.test(message)) {
        if (/^\s*}/.test(text)) {
            return { message: "There is an extra closing brace '}' here, or an earlier block was already closed." };
        }
        return { message: "C expected a declaration here. This statement looks like it is outside main; move it into main or remove the explicit main and let cBoxes add it." };
    }
    if (/^object type cannot be void or function$/.test(message)) {
        return { message: "You cannot make a variable with type void. Use void only for functions that return no value." };
    }
    if (/^array bound must be non-negative$/.test(message)) {
        return { message: "Array sizes cannot be negative. Use a positive whole number inside the brackets." };
    }
    if (/^object type must be complete$/.test(message) && /\[\s*0\s*\]/.test(text)) {
        return { message: "Array sizes must be positive. Use at least 1 inside the brackets." };
    }
    if (/^string literal is too long for the destination array$/.test(message)) {
        return { message: "This string is too long for the char array. Leave room for every character plus the final '\\0' byte." };
    }
    if (/^left shift count is negative or too large$/.test(message)) {
        return { message: "The shift amount must be between 0 and one less than the number of bits in the left value." };
    }
    if (/^signed integer overflow$/.test(message)) {
        return { message: "This integer calculation is too large for type int. Signed integer overflow is undefined in C." };
    }
    if (/^floating to integer conversion is outside the range of the destination type$/.test(message)) {
        return { message: "This decimal value is too large to fit in the destination integer type." };
    }
    const cannotConvert = /^cannot convert (.+) to (.+)$/.exec(message);
    if (cannotConvert) {
        return { message: `This value has type ${cannotConvert[1]}, but this location needs ${cannotConvert[2]}. Make the types match.` };
    }
    const undefinedFunction = /^call to undefined function ([A-Za-z_]\w*)$/.exec(message);
    if (undefinedFunction) {
        return { message: `cBoxes knows the name ${undefinedFunction[1]}, but this function is not available to run here. Check the include and function name.` };
    }
    if (/^read of uninitialized automatic object/.test(message)) {
        if (/\b[A-Za-z_]\w*\s*==/.test(text)) {
            return { message: "This reads a variable before it has a value. If you meant to assign a value, use = instead of ==." };
        }
        const derefName = dereferencedAssignmentName(text);
        if (derefName && declaredAsPointerBefore(lines, safeLine, derefName)) {
            return { message: `The pointer ${derefName} does not point anywhere yet. Set it to an address, like &a, before using *${derefName}.` };
        }
        if (derefName) {
            return { message: "The * operator only works on pointers. Make sure this variable has a pointer type before writing through *." };
        }
        if (/^\s*(?:int|double|float|char|short|long|bool)\s+\*\s*[A-Za-z_]\w*\s*=\s*[A-Za-z_]\w*\s*;/.test(text)) {
            return { message: "A pointer stores an address. Use & before the variable name to store its address, like int *p = &a;." };
        }
        return { message: "This reads a variable before it has been given a value. Assign it a value first." };
    }
    if (/^invalid operands to binary/.test(message)) {
        return { message: "This operator does not work with the types on its left and right sides." };
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
    const endCol = Math.max(col + 1, Math.floor(friendly.endCol ?? col + 1));
    return {
        kind: raw.kind,
        message: friendly.message,
        file: raw.file || undefined,
        range: {
            startLine: line,
            startCol: col,
            endLine: line,
            endCol,
        },
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
    return {
        kind: kinds.has(kind) ? kind : "unknown",
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
