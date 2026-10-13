import type {
  BoxState,
  CTypeInfo,
  ProgramDiagnostic,
} from "./shared-core-utils.js";

export interface CSourceFile {
  path: string;
  source: string;
}

export interface CExecutionBudget {
  traceFunctions?: boolean;
  stepLimit: number;
  followingTraceLimit: number;
  traceLimit?: number;
}

interface CImplicitMainResult {
  implicitMainApplied?: boolean;
  implicitMainNotice?: string | null;
}

export type CProgramResult = (
  | {
      kind: "ok";
      state: BoxState[];
      // Globals and main's parameters, as main starts, before any line runs.
      initialState: BoxState[];
      trace: CProgramTraceEvent[];
      mainClose: CProgramSourceLocation | null;
      blocked: CProgramBlocked | null;
      executionLimit: CProgramExecutionLimit | null;
      stdout: string;
      stdoutBytes: Uint8Array;
      stderrBytes: Uint8Array;
      stderr: string;
      stdinPosition: number;
      exitStatus: number;
    }
  | { kind: "compile" | "ub"; diagnostic: ProgramDiagnostic }
) &
  CImplicitMainResult;

export interface CProgramTraceEvent {
  callDepth: number;
  frameId: number;
  function: string;
  stdoutLength: number;
  stderrLength: number;
  stdinPosition: number;
  kind: string;
  file: string;
  startLine: number;
  endLine: number;
  state: BoxState[];
  skippedRange: CProgramSourceRange | null;
}

export interface CProgramSourceRange {
  file: string;
  startLine: number;
  startColumn: number;
  endLine: number;
  endColumn: number;
}

export interface CProgramSourceLocation {
  file: string;
  line: number;
}

export interface CProgramBlocked {
  file: string;
  startLine: number;
  endLine: number;
  function: string;
  state: BoxState[];
}

export interface CProgramExecutionLimit {
  reason: "repeating-loop" | "unreachable-loop-exit" | "step-limit" | "trace-limit";
  executedSteps: number;
  file: string;
  startLine: number;
  endLine: number;
  tracePosition: number;
}

export type CExpressionResult =
  | {
      kind: "ok";
      result: {
        kind: string;
        type: string;
        value: string;
        displayValue: string;
        exactValue: string;
        address: string;
        name?: string;
        valueLiteral?: {
          kind: "integer" | "floating";
          hasSuffix: boolean;
        } | null;
        typeInfo: CTypeInfo;
      };
    }
  | { kind: "compile" | "ub"; diagnostic: ProgramDiagnostic };
