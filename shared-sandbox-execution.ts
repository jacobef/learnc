import { runCFiles, type CExecutionBudget, type CProgramResult, type CSourceFile } from "./shared-c-interpreter.js";

export const DEFAULT_SANDBOX_STEP_LIMIT = 100_000;
export function sandboxStepLimit(value: number): number {
  return Number.isFinite(value) ? Math.max(1, Math.min(1_000_000, Math.floor(value))) : DEFAULT_SANDBOX_STEP_LIMIT;
}

/** Keep one exact execution prefix; extend it only when the user reaches its end. */
export class SandboxExecution {
  private traceLimit = 512;
  private followingTraceLimit = 256;
  private key = "";
  private result: CProgramResult | undefined;

  constructor(public stepLimit = DEFAULT_SANDBOX_STEP_LIMIT, private readonly execute = runCFiles) {}

  reset() {
    this.traceLimit = 512;
    this.followingTraceLimit = 256;
  }

  extend() {
    this.traceLimit += 512;
    this.followingTraceLimit += 256;
  }

  includePosition(position: number) {
    this.traceLimit = Math.max(this.traceLimit, Math.ceil((position + 1) / 512) * 512);
  }

  budget(): CExecutionBudget {
    return { stepLimit: this.stepLimit, followingTraceLimit: this.followingTraceLimit, traceLimit: this.traceLimit };
  }

  run(files: CSourceFile[], stdin: string, implicitMain: boolean): CProgramResult {
    const budget = this.budget();
    const key = JSON.stringify([files, stdin, implicitMain, budget]);
    if (key !== this.key || !this.result) {
      this.result = this.execute(files, undefined, stdin, implicitMain, budget);
      this.key = key;
    }
    return this.result;
  }
}
