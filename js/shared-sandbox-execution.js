import { runCFiles } from "./shared-c-interpreter.js";
/** Preserve the valid prefix and add a terminal step for attempting the failing operation. */
export function sandboxTimeline(result) {
    const context = result.kind === "ub" ? result.diagnostic.runtimeContext : null;
    if (result.kind !== "ub" || !context)
        return result;
    const diagnostic = result.diagnostic;
    return {
        kind: "ok",
        runtimeDiagnostic: diagnostic,
        implicitMainApplied: result.implicitMainApplied,
        implicitMainNotice: result.implicitMainNotice,
        state: context.state,
        trace: [...context.trace, {
                kind: "ub",
                callDepth: context.trace[context.trace.length - 1]?.callDepth ?? 0,
                frameId: context.trace[context.trace.length - 1]?.frameId ?? 0,
                function: context.trace[context.trace.length - 1]?.function ?? "main",
                file: diagnostic.file || context.trace[context.trace.length - 1]?.file || "program.c",
                startLine: diagnostic.range.startLine,
                endLine: diagnostic.range.endLine,
                state: [], // There is no program state after undefined behavior.
                stdoutLength: context.stdoutBytes.length,
                stderrLength: context.stderrBytes.length,
                stdinPosition: context.stdinPosition,
                skippedRange: null,
            }],
        mainClose: null,
        blocked: null,
        executionLimit: null,
        stdout: new TextDecoder().decode(context.stdoutBytes),
        stderr: new TextDecoder().decode(context.stderrBytes),
        stdoutBytes: context.stdoutBytes,
        stderrBytes: context.stderrBytes,
        stdinPosition: context.stdinPosition,
        exitStatus: 0,
    };
}
export function sandboxDiagnosticAt(result, position) {
    if (result.kind !== "ok")
        return result.diagnostic;
    return position >= result.trace.length ? result.runtimeDiagnostic ?? null : null;
}
export const DEFAULT_SANDBOX_STEP_LIMIT = 100000;
export function sandboxStepLimit(value) {
    return Number.isFinite(value) ? Math.max(1, Math.min(1000000, Math.floor(value))) : DEFAULT_SANDBOX_STEP_LIMIT;
}
/** Keep one exact execution prefix; extend it only when the user reaches its end. */
export class SandboxExecution {
    constructor(stepLimit = DEFAULT_SANDBOX_STEP_LIMIT, execute = runCFiles) {
        this.stepLimit = stepLimit;
        this.execute = execute;
        this.traceLimit = 512;
        this.followingTraceLimit = 256;
        this.key = "";
    }
    reset() {
        this.traceLimit = 512;
        this.followingTraceLimit = 256;
    }
    extend() {
        this.traceLimit += 512;
        this.followingTraceLimit += 256;
    }
    includePosition(position) {
        this.traceLimit = Math.max(this.traceLimit, Math.ceil((position + 1) / 512) * 512);
    }
    budget() {
        return { traceFunctions: true, stepLimit: this.stepLimit, followingTraceLimit: this.followingTraceLimit, traceLimit: this.traceLimit };
    }
    run(files, stdin, implicitMain) {
        const budget = this.budget();
        const key = JSON.stringify([files, stdin, implicitMain, budget]);
        if (key !== this.key || !this.result) {
            this.result = this.execute(files, undefined, stdin, implicitMain, budget);
            this.key = key;
        }
        return this.result;
    }
}
