import { runCFiles } from "./shared-c-interpreter.js";
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
        return { stepLimit: this.stepLimit, followingTraceLimit: this.followingTraceLimit, traceLimit: this.traceLimit };
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
