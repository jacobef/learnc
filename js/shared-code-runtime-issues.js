function ordinal(value) {
    const remainder100 = value % 100;
    if (remainder100 >= 11 && remainder100 <= 13)
        return `${value}th`;
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
export function diagnosticRuntimeContextText(diagnostic) {
    const context = diagnostic.runtimeContext;
    if (!context)
        return null;
    const steps = context.executedSteps.toLocaleString();
    if (context.lineExecutionCount !== null) {
        return `This happened on the ${ordinal(context.lineExecutionCount)} execution of this line, after ${steps} interpreted steps. The state below is from the last completed line before the failure.`;
    }
    return `This happened after ${steps} interpreted steps. The state below is from the last completed line before the failure.`;
}
export function appendDiagnosticRuntimeContext(container, diagnostic) {
    const text = diagnosticRuntimeContextText(diagnostic);
    if (!text)
        return;
    const context = document.createElement("div");
    context.className = "code-diagnostic-tip code-diagnostic-runtime-context";
    context.textContent = text;
    container.appendChild(context);
}
function lineRange(startLine, endLine) {
    const safeStartLine = Math.max(0, startLine);
    return {
        startLine: safeStartLine,
        startCol: 0,
        endLine: Math.max(safeStartLine, endLine),
        endCol: 1,
    };
}
export function codeRuntimeIssue(result) {
    if (result.kind !== "ok")
        return null;
    if (result.executionLimit) {
        return {
            kind: "execution-limit",
            title: "Program did not finish",
            status: "program did not finish",
            message: "The interpreter stopped this program because it ran for too many steps. This usually means an infinite loop or recursion that never reaches a base case. The state below is rewound to just before the repeating section.",
            tip: "Check whether a loop condition can become false or a recursive call can reach its base case.",
            range: lineRange(result.executionLimit.startLine, result.executionLimit.endLine),
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
            tip: "Remove the input operation or initialize the values directly in your code.",
            range: lineRange(result.blocked.startLine, result.blocked.endLine),
        };
    }
    return null;
}
export function offsetCodeRuntimeIssue(issue, lineDelta) {
    return {
        ...issue,
        range: lineRange(issue.range.startLine + lineDelta, issue.range.endLine + lineDelta),
    };
}
export function renderCodeRuntimeIssue(container, editor, issue, displayedLineOffset = 0) {
    if (!container)
        return;
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
    heading.textContent = `${issue.title} near line ${issue.range.startLine + displayedLineOffset + 1}`;
    const message = document.createElement("div");
    message.className = "code-diagnostic-message";
    message.textContent = issue.message;
    const tip = document.createElement("div");
    tip.className = "code-diagnostic-tip";
    tip.textContent = issue.tip;
    container.append(heading, message, tip);
    editor?.setAttribute("aria-invalid", "true");
}
