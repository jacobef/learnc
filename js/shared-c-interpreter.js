import { pageSyntheticAddressBase } from "./shared-c-address-space.js";
import { encodeSourceFiles, invokeCInterpreter, resetCInterpreterBridge, } from "./shared-c-wasm-bridge.js";
import { crashExpressionDiagnostic, crashProgramDiagnostic, interpreterDiagnosticFile, normalizeExpressionResult, normalizeProgramResult, } from "./shared-c-result-normalization.js";
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
export function runCProgram(source, addressBase = pageSyntheticAddressBase, stdin = "") {
    try {
        const parsed = invokeCInterpreter({
            operation: "run-source",
            primary: source,
            stdin,
            syntheticAddressBase: addressBase,
            executionBudget: DEFAULT_EXECUTION_BUDGET,
        });
        return normalizeProgramResult(parsed, source);
    }
    catch (error) {
        resetCInterpreterBridge();
        return crashProgramDiagnostic(error);
    }
}
export function runCFiles(files, addressBase = pageSyntheticAddressBase, stdin = "", implicitMain = true, executionBudget = DEFAULT_EXECUTION_BUDGET) {
    const bundle = encodeSourceFiles(files);
    const budget = normalizeExecutionBudget(executionBudget);
    try {
        const parsed = invokeCInterpreter({
            operation: "run-files",
            primary: bundle,
            stdin,
            syntheticAddressBase: addressBase,
            implicitMain,
            executionBudget: budget,
        });
        const diagnosticFile = interpreterDiagnosticFile(parsed, true);
        const diagnosticSource = diagnosticFile !== null
            ? files.find((file) => file.path === diagnosticFile)?.source ??
                files[0]?.source ??
                ""
            : files[0]?.source ?? "";
        return normalizeProgramResult(parsed, diagnosticSource, true);
    }
    catch (error) {
        resetCInterpreterBridge();
        return crashProgramDiagnostic(error);
    }
}
export function evaluateCExpression(source, eventIndex, expression, addressBase = pageSyntheticAddressBase, stdin = "") {
    try {
        const parsed = invokeCInterpreter({
            operation: "evaluate-source",
            primary: source,
            expression,
            stdin,
            syntheticAddressBase: addressBase,
            eventIndex: Math.max(0, Math.floor(eventIndex)),
            executionBudget: DEFAULT_EXECUTION_BUDGET,
        });
        return normalizeExpressionResult(parsed, expression);
    }
    catch (error) {
        resetCInterpreterBridge();
        return crashExpressionDiagnostic(error);
    }
}
export function evaluateCExpressionFiles(files, eventIndex, expression, addressBase = pageSyntheticAddressBase, stdin = "", implicitMain = true, executionBudget = DEFAULT_EXECUTION_BUDGET) {
    const bundle = encodeSourceFiles(files);
    const budget = normalizeExecutionBudget(executionBudget);
    try {
        const parsed = invokeCInterpreter({
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
        const diagnosticSource = diagnosticFile !== null && diagnosticFile !== "<expression>"
            ? files.find((file) => file.path === diagnosticFile)?.source ??
                files[0]?.source ??
                ""
            : expression;
        return normalizeExpressionResult(parsed, diagnosticSource, true);
    }
    catch (error) {
        resetCInterpreterBridge();
        return crashExpressionDiagnostic(error);
    }
}
