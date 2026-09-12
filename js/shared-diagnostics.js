const PRIMARY_DIAGNOSTIC_ANNOTATION_ID = "primary";
const diagnosticGroupIds = new WeakMap();
let nextDiagnosticGroupId = 1;
function diagnosticGroupId(diagnostic) {
    const existing = diagnosticGroupIds.get(diagnostic);
    if (existing)
        return existing;
    const created = `diagnostic-${nextDiagnosticGroupId}`;
    nextDiagnosticGroupId += 1;
    diagnosticGroupIds.set(diagnostic, created);
    return created;
}
function resolvedAnnotations(diagnostic) {
    const primary = {
        id: PRIMARY_DIAGNOSTIC_ANNOTATION_ID,
        file: diagnostic.file,
        range: diagnostic.range,
        colorIndex: 0,
    };
    const seen = new Set([PRIMARY_DIAGNOSTIC_ANNOTATION_ID]);
    const related = [];
    for (const annotation of diagnostic.annotations || []) {
        if (!annotation.id || seen.has(annotation.id))
            continue;
        seen.add(annotation.id);
        related.push({
            ...annotation,
            colorIndex: related.length + 1,
        });
    }
    return [primary, ...related];
}
function annotationClass(colorIndex) {
    return colorIndex === 0
        ? "code-highlight-error code-diagnostic-color-primary"
        : `code-highlight-related code-diagnostic-color-${Math.min(colorIndex, 3)}`;
}
function messageAnnotationClass(colorIndex) {
    return colorIndex === 0
        ? "code-diagnostic-message-link code-diagnostic-color-primary"
        : `code-diagnostic-message-link code-diagnostic-color-${Math.min(colorIndex, 3)}`;
}
function rangeForLine(range, line, lineLength) {
    return {
        startCol: line === range.startLine ? range.startCol : 0,
        endCol: line === range.endLine ? range.endCol : lineLength,
    };
}
export function diagnosticDecorations(diagnostic, lines, activeFile) {
    if (!diagnostic)
        return [];
    const groupId = diagnosticGroupId(diagnostic);
    const decorations = [];
    for (const annotation of resolvedAnnotations(diagnostic)) {
        if (annotation.file && activeFile && annotation.file !== activeFile)
            continue;
        const startLine = Math.max(0, annotation.range.startLine);
        const endLine = Math.min(lines.length - 1, Math.max(startLine, annotation.range.endLine));
        for (let line = startLine; line <= endLine; line += 1) {
            const lineRange = rangeForLine(annotation.range, line, lines[line]?.length ?? 0);
            decorations.push({
                line,
                ...lineRange,
                className: annotationClass(annotation.colorIndex),
                priority: annotation.colorIndex === 0 ? 100 : 100 - annotation.colorIndex,
                diagnosticAnnotationId: annotation.id,
                diagnosticGroupId: groupId,
            });
        }
    }
    return decorations;
}
export function diagnosticMessageText(diagnostic) {
    return diagnostic.messageParts?.map((part) => part.text).join("")
        || diagnostic.message;
}
export function renderDiagnosticMessage(container, diagnostic) {
    const groupId = diagnosticGroupId(diagnostic);
    const parts = diagnostic.messageParts?.length
        ? diagnostic.messageParts
        : [{ text: diagnostic.message }];
    const annotationById = new Map(resolvedAnnotations(diagnostic).map((annotation) => [annotation.id, annotation]));
    for (const part of parts) {
        const annotation = part.annotationId
            ? annotationById.get(part.annotationId)
            : undefined;
        if (!annotation) {
            container.appendChild(document.createTextNode(part.text));
            continue;
        }
        const marked = document.createElement("span");
        marked.className = messageAnnotationClass(annotation.colorIndex);
        marked.dataset.diagnosticAnnotation = annotation.id;
        marked.dataset.diagnosticGroup = groupId;
        marked.tabIndex = 0;
        marked.setAttribute("aria-label", `${part.text}. Highlights the corresponding code.`);
        marked.addEventListener("pointerenter", () => {
            marked.dataset.diagnosticHovered = "true";
            updateDiagnosticGroupEmphasis(groupId);
        });
        marked.addEventListener("pointerleave", () => {
            delete marked.dataset.diagnosticHovered;
            updateDiagnosticGroupEmphasis(groupId);
        });
        marked.addEventListener("focus", () => {
            updateDiagnosticGroupEmphasis(groupId);
        });
        marked.addEventListener("blur", () => {
            updateDiagnosticGroupEmphasis(groupId);
        });
        marked.textContent = part.text;
        container.appendChild(marked);
    }
}
function updateDiagnosticGroupEmphasis(groupId) {
    const groupElements = Array.from(document.querySelectorAll("[data-diagnostic-group][data-diagnostic-annotation]")).filter((element) => element.dataset.diagnosticGroup === groupId);
    const labels = groupElements.filter((element) => element.classList.contains("code-diagnostic-message-link"));
    const activeLabel = labels.find((element) => element.dataset.diagnosticHovered === "true")
        || labels.find((element) => element === document.activeElement);
    const activeAnnotationId = activeLabel?.dataset.diagnosticAnnotation;
    for (const element of groupElements) {
        const matches = element.dataset.diagnosticAnnotation === activeAnnotationId;
        element.classList.toggle("is-diagnostic-annotation-active", !!activeAnnotationId && matches);
        element.classList.toggle("is-diagnostic-annotation-muted", !!activeAnnotationId && !matches);
    }
}
export function offsetDiagnosticLines(diagnostic, lineOffset) {
    const shiftRange = (range) => ({
        ...range,
        startLine: range.startLine + lineOffset,
        endLine: range.endLine + lineOffset,
    });
    return {
        ...diagnostic,
        range: shiftRange(diagnostic.range),
        annotations: diagnostic.annotations?.map((annotation) => ({
            ...annotation,
            range: shiftRange(annotation.range),
        })),
    };
}
