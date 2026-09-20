import { pageSyntheticAddressBase } from "./shared-c-address-space.js";
import { runCFiles } from "./shared-c-interpreter.js";
import { executionBoundaries } from "./shared-sandbox-navigation.js";
/** Runs in a worker: only source locations cross back to the interactive page. */
export function discoverSandboxBoundaries(files, stdin, implicitMain, stepLimit, traceLimit, addressBase = pageSyntheticAddressBase) {
    const result = runCFiles(files, addressBase, stdin, implicitMain, { stepLimit, traceLimit, followingTraceLimit: 256 });
    if (result.kind !== "ok")
        return { boundaries: [], more: false };
    const endFile = files.find(file => file.path === result.mainClose?.file) ?? files[0];
    return {
        boundaries: executionBoundaries(result.trace, result.mainClose, result.blocked, {
            file: endFile.path, line: endFile.source.split(/\r?\n/).length,
        }, result.executionLimit !== null),
        more: result.executionLimit?.reason === "trace-limit",
    };
}
