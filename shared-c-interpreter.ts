import { pageSyntheticAddressBase } from "./shared-c-address-space.js";
import {
  encodeSourceFiles,
  invokeCInterpreter,
  resetCInterpreterBridge,
} from "./shared-c-wasm-bridge.js";
import {
  crashExpressionDiagnostic,
  crashProgramDiagnostic,
  interpreterDiagnosticFile,
  normalizeExpressionResult,
  normalizeProgramResult,
} from "./shared-c-result-normalization.js";
import type {
  CExecutionBudget,
  CExpressionResult,
  CProgramResult,
  CSourceFile,
} from "./shared-c-interpreter-types.js";

export type {
  CExecutionBudget,
  CExpressionResult,
  CProgramBlocked,
  CProgramExecutionLimit,
  CProgramResult,
  CProgramSourceLocation,
  CProgramSourceRange,
  CProgramTraceEvent,
  CSourceFile,
} from "./shared-c-interpreter-types.js";
const DEFAULT_EXECUTION_BUDGET: CExecutionBudget = {
  stepLimit: 10_000,
  followingTraceLimit: 256,
};

function normalizeExecutionBudget(
  budget: CExecutionBudget,
): CExecutionBudget {
  return {
    stepLimit: Math.max(1, Math.floor(budget.stepLimit)),
    followingTraceLimit: Math.max(1, Math.floor(budget.followingTraceLimit)),
  };
}

export function runCProgram(
  source: string,
  addressBase: number = pageSyntheticAddressBase,
  stdin: string = "",
): CProgramResult {
  try {
    const parsed = invokeCInterpreter<unknown>({
      operation: "run-source",
      primary: source,
      stdin,
      syntheticAddressBase: addressBase,
      executionBudget: DEFAULT_EXECUTION_BUDGET,
    });
    return normalizeProgramResult(parsed, source);
  } catch (error) {
    resetCInterpreterBridge();
    return crashProgramDiagnostic(error);
  }
}

export function runCFiles(
  files: CSourceFile[],
  addressBase: number = pageSyntheticAddressBase,
  stdin: string = "",
  implicitMain: boolean = true,
  executionBudget: CExecutionBudget = DEFAULT_EXECUTION_BUDGET,
): CProgramResult {
  const bundle = encodeSourceFiles(files);
  const budget = normalizeExecutionBudget(executionBudget);
  try {
    const parsed = invokeCInterpreter<unknown>({
      operation: "run-files",
      primary: bundle,
      stdin,
      syntheticAddressBase: addressBase,
      implicitMain,
      executionBudget: budget,
    });
    const diagnosticFile = interpreterDiagnosticFile(parsed, true);
    const diagnosticSource =
      diagnosticFile !== null
        ? files.find((file) => file.path === diagnosticFile)?.source ??
          files[0]?.source ??
          ""
        : files[0]?.source ?? "";
    return normalizeProgramResult(parsed, diagnosticSource, true);
  } catch (error) {
    resetCInterpreterBridge();
    return crashProgramDiagnostic(error);
  }
}

export function evaluateCExpression(
  source: string,
  eventIndex: number,
  expression: string,
  addressBase: number = pageSyntheticAddressBase,
  stdin: string = "",
): CExpressionResult {
  try {
    const parsed = invokeCInterpreter<unknown>({
      operation: "evaluate-source",
      primary: source,
      expression,
      stdin,
      syntheticAddressBase: addressBase,
      eventIndex: Math.max(0, Math.floor(eventIndex)),
      executionBudget: DEFAULT_EXECUTION_BUDGET,
    });
    return normalizeExpressionResult(parsed, expression);
  } catch (error) {
    resetCInterpreterBridge();
    return crashExpressionDiagnostic(error);
  }
}

export function evaluateCExpressionFiles(
  files: CSourceFile[],
  eventIndex: number,
  expression: string,
  addressBase: number = pageSyntheticAddressBase,
  stdin: string = "",
  implicitMain: boolean = true,
  executionBudget: CExecutionBudget = DEFAULT_EXECUTION_BUDGET,
): CExpressionResult {
  const bundle = encodeSourceFiles(files);
  const budget = normalizeExecutionBudget(executionBudget);
  try {
    const parsed = invokeCInterpreter<unknown>({
      operation: "evaluate-files",
      primary: bundle,
      expression,
      stdin,
      syntheticAddressBase: addressBase,
      eventIndex: Math.max(0, Math.floor(eventIndex)),
      implicitMain,
      executionBudget: budget,
    });
    const diagnosticFile = interpreterDiagnosticFile(parsed, true);
    const diagnosticSource =
      diagnosticFile !== null && diagnosticFile !== "<expression>"
        ? files.find((file) => file.path === diagnosticFile)?.source ??
          files[0]?.source ??
          ""
        : expression;
    return normalizeExpressionResult(parsed, diagnosticSource, true);
  } catch (error) {
    resetCInterpreterBridge();
    return crashExpressionDiagnostic(error);
  }
}
