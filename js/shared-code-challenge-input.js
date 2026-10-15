import { runCFiles, runCProgram } from "./shared-c-interpreter.js";
import { diagnosticMessageText } from "./shared-diagnostics.js";
/** A declaration of name with the given type, e.g. int[3] -> "int name[3]". */
export function declaratorFor(type, name) {
    const array = /^(.*?)\s*((?:\[\s*\d*\s*\])+)$/.exec(type.trim());
    return array ? `${array[1]} ${name}${array[2]}` : `${type.trim()} ${name}`;
}
// The elements of a brace list like {1, 2, 3}.
function braceListElements(literal) {
    const inner = /^\{([\s\S]*)\}$/.exec(literal.trim())?.[1];
    if (inner === undefined)
        throw new Error("must be a brace-enclosed list of elements.");
    const elements = [];
    let depth = 0;
    let current = "";
    for (const ch of inner) {
        if (ch === "," && depth === 0) {
            elements.push(current.trim());
            current = "";
            continue;
        }
        if (ch === "{" || ch === "(")
            depth += 1;
        if (ch === "}" || ch === ")")
            depth -= 1;
        current += ch;
    }
    if (current.trim())
        elements.push(current.trim());
    return elements;
}
/** The locked code that creates an input with the given C literal. */
export function inputDeclaration(spec, literal) {
    if (!spec.allocated)
        return `${declaratorFor(spec.type, spec.name)} = ${literal};\n`;
    const element = spec.type.trim().replace(/\s*\*$/, "");
    const elements = braceListElements(literal);
    const fill = elements.map((value, index) => `${spec.name}[${index}] = ${value};`).join(" ");
    return `${element}* ${spec.name} = malloc(${elements.length} * sizeof(${element}));\n${fill ? `${fill}\n` : ""}`;
}
function withTrailingNewline(text) {
    return text === "" || text.endsWith("\n") ? text : `${text}\n`;
}
export function challengeFileName(shape) {
    return shape.fileName ?? "program.c";
}
export function challengeImplicitMain(shape) {
    return shape.implicitMain ?? !shape.main;
}
/** The locked code above the learner's code. */
export function challengePrefix(shape, literals) {
    const declarations = !challengeImplicitMain(shape)
        ? ""
        : shape.inputs.map((spec, index) => inputDeclaration(spec, literals[index])).join("");
    const prelude = shape.prelude ?? "";
    const needsStdlib = shape.inputs.some((spec) => spec.allocated) && !prelude.includes("#include <stdlib.h>");
    return (needsStdlib ? "#include <stdlib.h>\n" : "") + withTrailingNewline(prelude) + declarations;
}
/** The locked code below the learner's code. */
export function challengeSuffix(shape, literals) {
    return shape.main ? withTrailingNewline(shape.main(literals)) : "";
}
export function runChallengeProgram(shape, literals, body) {
    // The learner's code takes exactly as many lines as the editor shows, so
    // locked code after it keeps the line numbers it's displayed with.
    const source = challengePrefix(shape, literals) +
        `${body.replace(/\r\n/g, "\n")}\n` +
        challengeSuffix(shape, literals);
    const implicitMain = challengeImplicitMain(shape);
    if (implicitMain && !shape.files?.length)
        return runCProgram(source);
    return runCFiles([...(shape.files ?? []), { path: challengeFileName(shape), source }], undefined, "", implicitMain);
}
/** Convert lesson inputs with the interpreter's C type rules. */
export function normalizeChallengeInput(value, spec) {
    const literal = value.trim();
    const { type } = spec;
    if (spec.allocated) {
        const run = runCProgram(`#include <stdlib.h>\n${inputDeclaration({ ...spec, name: "cboxes_input" }, literal)}`);
        if (run.kind !== "ok")
            throw new Error(`is not valid C: ${diagnosticMessageText(run.diagnostic)}`);
        return { runtime: literal, literal };
    }
    // This isolated probe must remain visible in the returned program state.
    // The interpreter reserves __cboxes_* for hidden implementation objects.
    const name = "cboxes_input";
    const run = runCProgram(`${declaratorFor(type, name)} = ${literal};\n`);
    if (run.kind !== "ok")
        throw new Error(`is not valid C: ${diagnosticMessageText(run.diagnostic)}`);
    const stored = run.state.find(box => box.name === name);
    if (!stored)
        throw new Error("did not create a C object.");
    // An array's value is its initializer, as written.
    return { runtime: stored.arrayShape?.length ? literal : stored.value, literal };
}
