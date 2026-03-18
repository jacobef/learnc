function textValueForLine(line) {
    return line === "" ? " " : line;
}
function clampColumn(value, lineLength) {
    if (!Number.isFinite(value))
        return 0;
    return Math.max(0, Math.min(lineLength, Math.floor(value)));
}
function normalizedDecorationsForLine(line, decorations) {
    const lineLength = line.length;
    const normalized = decorations
        .map((decoration) => {
        let startCol = clampColumn(decoration.startCol, lineLength);
        let endCol = clampColumn(decoration.endCol, lineLength);
        if (endCol <= startCol) {
            if (lineLength > 0) {
                startCol = Math.max(0, Math.min(startCol, lineLength - 1));
                endCol = Math.min(lineLength, startCol + 1);
            }
            else {
                startCol = 0;
                endCol = 1;
            }
        }
        return {
            ...decoration,
            startCol,
            endCol,
        };
    })
        .sort((left, right) => left.startCol - right.startCol || left.endCol - right.endCol);
    const out = [];
    for (const decoration of normalized) {
        const prev = out[out.length - 1];
        if (!prev || decoration.startCol >= prev.endCol) {
            out.push(decoration);
            continue;
        }
        if (decoration.endCol <= prev.endCol)
            continue;
        out.push({
            ...decoration,
            startCol: prev.endCol,
        });
    }
    return out;
}
function appendLineContent(lineEl, line, decorations) {
    const normalizedLine = textValueForLine(line);
    if (!decorations.length) {
        lineEl.textContent = normalizedLine;
        return;
    }
    const normalizedDecorations = normalizedDecorationsForLine(line, decorations);
    let cursor = 0;
    for (const decoration of normalizedDecorations) {
        const start = clampColumn(decoration.startCol, line.length);
        const end = clampColumn(decoration.endCol, Math.max(line.length, 1));
        if (start > cursor) {
            lineEl.appendChild(document.createTextNode(textValueForLine(line.slice(cursor, start))));
        }
        const marked = document.createElement("span");
        marked.className = decoration.className;
        marked.textContent = textValueForLine(line.slice(start, end));
        lineEl.appendChild(marked);
        cursor = Math.max(cursor, end);
    }
    if (cursor < line.length) {
        lineEl.appendChild(document.createTextNode(line.slice(cursor)));
    }
    if (!lineEl.childNodes.length) {
        lineEl.textContent = normalizedLine;
    }
}
export function ensureCodeSurfaceElements(editor) {
    if (!editor?.parentElement) {
        return { highlightEl: null, measureEl: null };
    }
    let highlightEl = editor.parentElement.querySelector(".code-textarea-highlight");
    if (!highlightEl) {
        highlightEl = document.createElement("pre");
        highlightEl.className = "code-textarea-highlight";
        highlightEl.setAttribute("aria-hidden", "true");
        editor.parentElement.insertBefore(highlightEl, editor);
    }
    let measureEl = editor.parentElement.querySelector(".code-textarea-measure");
    if (!measureEl) {
        measureEl = document.createElement("div");
        measureEl.className = "code-textarea-measure";
        measureEl.setAttribute("aria-hidden", "true");
        editor.parentElement.appendChild(measureEl);
    }
    return { highlightEl, measureEl };
}
export function getCodeLineHeightPx(editor) {
    if (!editor)
        return 32;
    const style = window.getComputedStyle(editor);
    const value = parseFloat(style.lineHeight);
    return Number.isFinite(value) ? value : 32;
}
export function autoSizeCodeEditor(editor) {
    if (!editor)
        return;
    editor.style.height = "auto";
    editor.style.height = `${editor.scrollHeight}px`;
}
export function measureCodeWrapCounts(editor, measureEl, lines) {
    if (!editor || !measureEl)
        return lines.map(() => 1);
    const style = window.getComputedStyle(editor);
    const paddingLeft = parseFloat(style.paddingLeft) || 0;
    const paddingRight = parseFloat(style.paddingRight) || 0;
    const contentWidth = Math.max(1, editor.clientWidth - paddingLeft - paddingRight);
    measureEl.style.width = `${contentWidth}px`;
    measureEl.style.fontFamily = style.fontFamily;
    measureEl.style.fontSize = style.fontSize;
    measureEl.style.fontWeight = style.fontWeight;
    measureEl.style.letterSpacing = style.letterSpacing;
    measureEl.style.lineHeight = style.lineHeight;
    const lineHeight = getCodeLineHeightPx(editor);
    return lines.map((line) => {
        measureEl.textContent = textValueForLine(line);
        const height = measureEl.scrollHeight;
        return Math.max(1, Math.ceil(height / lineHeight - 0.01));
    });
}
export function updateCodeSurface({ editor, lineNumbers = null, highlightEl = null, measureEl = null, lines, lineNumberStart = 1, lineClasses, lineNumberClasses, decorations = [], }) {
    autoSizeCodeEditor(editor);
    const safeLines = lines.length ? lines : [""];
    const lineHeight = getCodeLineHeightPx(editor);
    const wraps = measureCodeWrapCounts(editor, measureEl, safeLines);
    if (highlightEl) {
        highlightEl.replaceChildren();
        const perLineDecorations = new Map();
        for (const decoration of decorations) {
            const line = Math.floor(Number(decoration.line));
            if (!Number.isFinite(line) || line < 0 || line >= safeLines.length)
                continue;
            const bucket = perLineDecorations.get(line) || [];
            bucket.push(decoration);
            perLineDecorations.set(line, bucket);
        }
        const frag = document.createDocumentFragment();
        safeLines.forEach((line, index) => {
            const lineEl = document.createElement("span");
            lineEl.className = "code-highlight-line";
            const extraLineClasses = lineClasses?.get(index) || [];
            if (extraLineClasses.length)
                lineEl.classList.add(...extraLineClasses);
            appendLineContent(lineEl, line, perLineDecorations.get(index) || []);
            frag.appendChild(lineEl);
            if (index < safeLines.length - 1) {
                frag.appendChild(document.createTextNode("\n"));
            }
        });
        highlightEl.appendChild(frag);
    }
    if (lineNumbers) {
        const frag = document.createDocumentFragment();
        safeLines.forEach((_line, index) => {
            const num = document.createElement("div");
            num.className = "code-line-number";
            num.style.height = `${(wraps[index] || 1) * lineHeight}px`;
            const extraLineNumberClasses = lineNumberClasses?.get(index) || [];
            if (extraLineNumberClasses.length)
                num.classList.add(...extraLineNumberClasses);
            num.textContent = String(lineNumberStart + index);
            frag.appendChild(num);
        });
        lineNumbers.replaceChildren(frag);
        if (editor) {
            lineNumbers.style.height = `${editor.clientHeight}px`;
            lineNumbers.scrollTop = editor.scrollTop;
        }
    }
    return { wraps, lineHeight };
}
export function bindCodeEditorTabKey(editor) {
    if (!editor || editor.dataset.codeTabBound === "1")
        return;
    editor.dataset.codeTabBound = "1";
    const indentUnit = "  ";
    const indentWidth = indentUnit.length;
    editor.addEventListener("keydown", (event) => {
        if (event.key !== "Tab" || event.altKey || event.ctrlKey || event.metaKey) {
            return;
        }
        event.preventDefault();
        const value = editor.value;
        const start = editor.selectionStart ?? 0;
        const end = editor.selectionEnd ?? start;
        const lineStart = value.lastIndexOf("\n", Math.max(0, start - 1)) + 1;
        const blockSelectionEnd = end > start && value[end - 1] === "\n" ? end - 1 : end;
        const lineEndBreak = value.indexOf("\n", blockSelectionEnd);
        const blockEnd = lineEndBreak === -1 ? value.length : lineEndBreak;
        const selectedText = value.slice(start, end);
        const preserveScrollTop = editor.scrollTop;
        const preserveScrollLeft = editor.scrollLeft;
        const applyEdit = (nextValue, nextSelectionStart, nextSelectionEnd) => {
            editor.value = nextValue;
            editor.setSelectionRange(nextSelectionStart, nextSelectionEnd);
            editor.scrollTop = preserveScrollTop;
            editor.scrollLeft = preserveScrollLeft;
            editor.dispatchEvent(new Event("input", { bubbles: true }));
        };
        if (event.shiftKey) {
            const lines = value.slice(lineStart, blockEnd).split("\n");
            const removedPerLine = lines.map((line) => {
                if (line.startsWith("\t"))
                    return 1;
                const spaceMatch = line.match(/^ {1,2}/);
                return spaceMatch ? spaceMatch[0].length : 0;
            });
            if (!removedPerLine.some((count) => count > 0))
                return;
            const outdented = lines
                .map((line, index) => line.slice(removedPerLine[index] || 0))
                .join("\n");
            const nextValue = value.slice(0, lineStart) + outdented + value.slice(blockEnd);
            if (start === end) {
                const nextPos = Math.max(lineStart, start - Math.min(removedPerLine[0] || 0, start - lineStart));
                applyEdit(nextValue, nextPos, nextPos);
                return;
            }
            const removedBeforeStart = Math.min(removedPerLine[0] || 0, Math.max(0, start - lineStart));
            const totalRemoved = removedPerLine.reduce((sum, count) => sum + count, 0);
            const nextStart = Math.max(lineStart, start - removedBeforeStart);
            const nextEnd = Math.max(nextStart, end - totalRemoved);
            applyEdit(nextValue, nextStart, nextEnd);
            return;
        }
        if (start !== end && selectedText.includes("\n")) {
            const lines = value.slice(lineStart, blockEnd).split("\n");
            const indented = lines.map((line) => `${indentUnit}${line}`).join("\n");
            const nextValue = value.slice(0, lineStart) + indented + value.slice(blockEnd);
            const nextStart = start + indentWidth;
            const nextEnd = end + lines.length * indentWidth;
            applyEdit(nextValue, nextStart, nextEnd);
            return;
        }
        const nextValue = value.slice(0, start) + indentUnit + value.slice(end);
        const nextPos = start + indentWidth;
        applyEdit(nextValue, nextPos, nextPos);
    });
}
