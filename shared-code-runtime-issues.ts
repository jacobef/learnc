import { renderDiagnosticMessage } from "./shared-diagnostics.js";
import type {
  ProgramDiagnostic,
  ProgramDiagnosticRange,
} from "./shared-core-utils.js";
import type { CProgramExecutionLimit, CProgramResult } from "./shared-c-interpreter-types.js";

export type CodeRuntimeIssueKind = "execution-limit" | "blocked-input";

export interface CodeRuntimeIssue {
  kind: CodeRuntimeIssueKind;
  title: string;
  status: string;
  message: string;
  tip: string;
  range: ProgramDiagnosticRange;
}

function ordinal(value: number): string {
  const remainder100 = value % 100;
  if (remainder100 >= 11 && remainder100 <= 13) return `${value}th`;
  switch (value % 10) {
    case 1:
      return `${value}st`;
    case 2:
      return `${value}nd`;
    case 3:
      return `${value}rd`;
    default:
      return `${value}th`;
  }
}

export function diagnosticRuntimeContextText(
  diagnostic: ProgramDiagnostic,
): string | null {
  const context = diagnostic.runtimeContext;
  if (!context) return null;
  const steps = context.executedSteps.toLocaleString();
  if (context.lineExecutionCount !== null) {
    return `This happened on the ${ordinal(
      context.lineExecutionCount,
    )} execution of this line, after ${steps} interpreted steps. The state below is from the last completed line before the failure.`;
  }
  return `This happened after ${steps} interpreted steps. The state below is from the last completed line before the failure.`;
}

function appendDiagnosticRuntimeContext(
  container: HTMLElement,
  diagnostic: ProgramDiagnostic,
): void {
  const text = diagnosticRuntimeContextText(diagnostic);
  if (!text) return;
  const context = document.createElement("div");
  context.className = "code-diagnostic-tip code-diagnostic-runtime-context";
  context.textContent = text;
  container.appendChild(context);
}

function lineRange(startLine: number, endLine: number): ProgramDiagnosticRange {
  const safeStartLine = Math.max(0, startLine);
  return {
    startLine: safeStartLine,
    startCol: 0,
    endLine: Math.max(safeStartLine, endLine),
    endCol: 1,
  };
}

export function executionLimitExplanation(limit: CProgramExecutionLimit) {
  if (limit.reason === "repeating-loop") {
    return {
      title: "Infinite loop",
      status: "infinite loop detected",
      message: "This loop runs forever.",
    };
  }
  if (limit.reason === "unreachable-loop-exit") {
    return {
      title: "Loop cannot reach its stopping condition",
      status: "non-terminating loop detected",
      message: "The counter's update cannot reach this loop's stopping condition. Check its direction, increment size, and comparison. A signed integer counter may eventually overflow instead of ending the loop. The stepper is rewound to just before the loop.",
    };
  }
  return {
    title: "Program did not finish",
    status: "program did not finish",
    message: `The interpreter paused after ${limit.executedSteps.toLocaleString()} steps. The program may need more time, or it may contain an infinite loop or recursion that never reaches a base case. The stepper is moved back so you can inspect the execution.`,
  };
}

export function codeRuntimeIssue(
  result: CProgramResult,
): CodeRuntimeIssue | null {
  if (result.kind !== "ok") return null;
  if (result.executionLimit) {
    return {
      kind: "execution-limit",
      ...executionLimitExplanation(result.executionLimit),
      tip:
        "Check whether a loop condition can become false or a recursive call can reach its base case.",
      range: lineRange(
        result.executionLimit.startLine,
        result.executionLimit.endLine,
      ),
    };
  }
  if (result.blocked) {
    const functionName = result.blocked.function.trim();
    return {
      kind: "blocked-input",
      title: "Program is waiting for input",
      status: "program is waiting for input",
      message: functionName
        ? `The program reached ${functionName}(), but this lesson has no more input available.`
        : "The program tried to read input, but this lesson has no more input available.",
      tip:
        "Remove the input operation or initialize the values directly in your code.",
      range: lineRange(result.blocked.startLine, result.blocked.endLine),
    };
  }
  return null;
}

export function offsetCodeRuntimeIssue(
  issue: CodeRuntimeIssue,
  lineDelta: number,
): CodeRuntimeIssue {
  return {
    ...issue,
    range: lineRange(
      issue.range.startLine + lineDelta,
      issue.range.endLine + lineDelta,
    ),
  };
}

export function renderCodeRuntimeIssue(
  container: HTMLElement | null,
  editor: HTMLTextAreaElement | null,
  issue: CodeRuntimeIssue | null,
  displayedLineOffset = 0,
): void {
  if (!container) return;
  if (!issue) {
    container.classList.add("hidden");
    container.textContent = "";
    editor?.removeAttribute("aria-invalid");
    return;
  }

  container.classList.remove("hidden");
  container.replaceChildren();
  const heading = document.createElement("div");
  heading.className = "code-diagnostic-title";
  heading.textContent = `${issue.title} near line ${
    issue.range.startLine + displayedLineOffset + 1
  }`;
  const message = document.createElement("div");
  message.className = "code-diagnostic-message";
  message.textContent = issue.message;
  const tip = document.createElement("div");
  tip.className = "code-diagnostic-tip";
  tip.textContent = issue.tip;
  container.append(heading, message, tip);
  editor?.setAttribute("aria-invalid", "true");
}

export function renderCodeDiagnostic(
  container: HTMLElement | null,
  editor: HTMLTextAreaElement | null,
  diagnostic: ProgramDiagnostic | null,
  {
    displayedLineOffset = 0,
    activeFile,
    showRuntimeContext = true,
  }: {
    displayedLineOffset?: number;
    activeFile?: string;
    showRuntimeContext?: boolean;
  } = {},
): void {
  const appliesToEditor = diagnostic &&
    (!activeFile || !diagnostic.file || diagnostic.file === activeFile);
  if (appliesToEditor) editor?.setAttribute("aria-invalid", "true");
  else editor?.removeAttribute("aria-invalid");
  if (!container) return;
  container.replaceChildren();
  container.classList.toggle("hidden", !diagnostic);
  if (!diagnostic) return;

  const heading = document.createElement("div");
  heading.className = "code-diagnostic-title";
  const location = `line ${diagnostic.range.startLine + displayedLineOffset + 1}, column ${diagnostic.range.startCol + 1}`;
  const filePrefix = activeFile && diagnostic.file ? `${diagnostic.file}, ` : "";
  heading.textContent = `${diagnostic.kind === "ub" ? "Undefined behavior" : "Error"} at ${filePrefix}${location}`;
  const message = document.createElement("div");
  message.className = "code-diagnostic-message";
  renderDiagnosticMessage(message, diagnostic);
  container.append(heading, message);
  if (diagnostic.tip) {
    const tip = document.createElement("div");
    tip.className = "code-diagnostic-tip";
    tip.textContent = diagnostic.tip;
    container.appendChild(tip);
  }
  if (showRuntimeContext) appendDiagnosticRuntimeContext(container, diagnostic);
}
