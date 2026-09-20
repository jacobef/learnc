import { runCProgram } from "./shared-c-interpreter.js";
import { diagnosticMessageText } from "./shared-diagnostics.js";
/** Convert lesson inputs with the interpreter's C type rules. */
export function normalizeChallengeInput(value, type) {
    const literal = value.trim();
    // This isolated probe must remain visible in the returned program state.
    // The interpreter reserves __cboxes_* for hidden implementation objects.
    const name = "cboxes_input";
    const run = runCProgram(`${type} ${name} = ${literal};\n`);
    if (run.kind !== "ok")
        throw new Error(`is not valid C: ${diagnosticMessageText(run.diagnostic)}`);
    const stored = run.state.find(box => box.name === name);
    if (!stored)
        throw new Error("did not create a scalar C value.");
    return { runtime: stored.value, literal };
}
