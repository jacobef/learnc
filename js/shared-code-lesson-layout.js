import { ensurePanelizedMain, isMobileViewport, queryElement, queryRole, syncDocumentTitleFromNav, } from "./shared-core.js";
import { disableAutoText } from "./shared-dom-utils.js";
function collectCodeLessonElements(root = document) {
    const role = (name) => queryRole(name, root);
    const rootElement = root instanceof HTMLElement ? root : null;
    return {
        instructionsEl: role("code-instructions"),
        lockedLineNumbers: role("code-locked-line-numbers"),
        lockedInputLine: role("code-locked-input-line"),
        lockedSuffixNumbers: role("code-locked-suffix-numbers"),
        lockedSuffixLines: role("code-locked-suffix-lines"),
        editor: role("code-editor"),
        lineNumbers: role("code-line-numbers"),
        stage: role("code-stage"),
        status: role("code-status"),
        diagnosticEl: role("code-diagnostic"),
        hintPanel: role("code-hint"),
        hintBtn: role("code-hint-btn"),
        checkBtn: role("code-check"),
        levelResetBtn: role("code-reset-level"),
        rerollBtn: role("code-reroll"),
        showFailBtn: role("code-show-failing-case"),
        prevBtn: queryElement('button[data-stepper="prev"]', root),
        nextBtn: queryElement('button[data-stepper="next"]', root),
        codeRoot: rootElement?.dataset.role === "code-root"
            ? rootElement
            : role("code-root"),
    };
}
function lessonButton({ text, role, stepper, className, }) {
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = text;
    if (role)
        button.dataset.role = role;
    if (stepper)
        button.dataset.stepper = stepper;
    if (className)
        button.className = className;
    return button;
}
function lockedCodeRow(numbersRole, linesRole) {
    const row = document.createElement("div");
    row.className = "codepane-row code-locked-row";
    const numbers = document.createElement("div");
    numbers.dataset.role = numbersRole;
    numbers.className = "code-gutter";
    numbers.setAttribute("aria-hidden", "true");
    const lines = document.createElement("div");
    lines.dataset.role = linesRole;
    lines.className = "code-locked-line";
    row.append(numbers, lines);
    return row;
}
function fileHeading(path) {
    const heading = document.createElement("div");
    heading.className = "code-file-name";
    heading.textContent = path;
    return heading;
}
function readOnlyFileBlock(file) {
    const block = document.createElement("div");
    block.className = "code-readonly-file";
    const row = document.createElement("div");
    row.className = "codepane-row code-locked-row";
    const numbers = document.createElement("div");
    numbers.className = "code-gutter";
    numbers.setAttribute("aria-hidden", "true");
    const lines = file.source.replace(/\n$/, "").split("\n");
    lines.forEach((_, index) => {
        const number = document.createElement("div");
        number.className = "code-line-number";
        number.textContent = String(index + 1);
        numbers.appendChild(number);
    });
    const text = document.createElement("div");
    text.className = "code-locked-line";
    text.textContent = lines.join("\n");
    row.append(numbers, text);
    block.append(fileHeading(file.path), row);
    return block;
}
function ensureCodeLessonLayout({ textareaMinLines, lockedInput, lockedSuffix = false, readOnlyFiles = [], fileName = "program.c", }) {
    const resolvedTitle = syncDocumentTitleFromNav();
    if (queryRole("code-editor")) {
        return collectCodeLessonElements();
    }
    const main = ensurePanelizedMain(resolvedTitle);
    const section = document.createElement("section");
    section.dataset.role = "code-root";
    section.className = "panel-shell";
    const actionBar = document.createElement("div");
    actionBar.className = "controls-bar controls-bar-code";
    const controlsMain = document.createElement("div");
    controlsMain.className = "controls-main panel panel-controls";
    const controlsRow = document.createElement("div");
    controlsRow.className = "controls-row controls-left";
    controlsMain.appendChild(controlsRow);
    actionBar.appendChild(controlsMain);
    const row = document.createElement("div");
    row.className = "row panel-row";
    section.append(actionBar, row);
    main.appendChild(section);
    const codePanel = document.createElement("div");
    codePanel.className = "panel code-editor-panel panel-scroll code-panel-shell";
    codePanel.dataset.role = "code-panel";
    const codeTitle = document.createElement("div");
    codeTitle.className = "panel-title code-title";
    codeTitle.textContent = "Code";
    const codePane = document.createElement("div");
    codePane.className = "codepane panel-body";
    for (const file of readOnlyFiles)
        codePane.appendChild(readOnlyFileBlock(file));
    if (readOnlyFiles.length)
        codePane.appendChild(fileHeading(fileName));
    if (lockedInput) {
        codePane.appendChild(lockedCodeRow("code-locked-line-numbers", "code-locked-input-line"));
    }
    const codeRow = document.createElement("div");
    codeRow.className = "codepane-row";
    const lineNumbers = document.createElement("div");
    lineNumbers.dataset.role = "code-line-numbers";
    lineNumbers.className = "code-gutter";
    lineNumbers.setAttribute("aria-hidden", "true");
    const editorWrap = document.createElement("div");
    editorWrap.className = "code-editor-wrap";
    const editor = document.createElement("textarea");
    editor.dataset.role = "code-editor";
    editor.className = "code-textarea";
    disableAutoText(editor);
    editor.setAttribute("aria-label", "C code");
    editor.rows = Math.max(1, Math.floor(textareaMinLines));
    editorWrap.appendChild(editor);
    codeRow.append(lineNumbers, editorWrap);
    codePane.appendChild(codeRow);
    if (lockedSuffix) {
        codePane.appendChild(lockedCodeRow("code-locked-suffix-numbers", "code-locked-suffix-lines"));
    }
    const diagnosticEl = document.createElement("div");
    diagnosticEl.dataset.role = "code-diagnostic";
    diagnosticEl.className = "code-diagnostic hidden";
    codePanel.append(codeTitle, codePane, diagnosticEl);
    const stateColumn = document.createElement("div");
    stateColumn.className = "code-editor-state-col";
    const stage = document.createElement("div");
    stage.dataset.role = "code-stage";
    stage.className = "code-editor-state-stage";
    stateColumn.appendChild(stage);
    row.append(codePanel, stateColumn);
    const previous = lessonButton({ text: "Back ◀", stepper: "prev" });
    const next = lessonButton({ text: "Next Program ▶▶", stepper: "next" });
    const spacer = document.createElement("span");
    spacer.className = "controls-spacer";
    spacer.setAttribute("aria-hidden", "true");
    controlsRow.append(previous, next, spacer);
    if (lockedInput) {
        controlsRow.appendChild(lessonButton({ text: "New input", role: "code-reroll" }));
    }
    controlsRow.append(lessonButton({ text: "Reset level", role: "code-reset-level" }), lessonButton({ text: "Hint", role: "code-hint-btn", className: "hint-link" }), lessonButton({ text: "Check", role: "code-check" }));
    if (lockedInput) {
        controlsRow.appendChild(lessonButton({
            text: "Show failing case",
            role: "code-show-failing-case",
            className: "hidden",
        }));
    }
    const status = document.createElement("span");
    status.dataset.role = "code-status";
    status.className = "muted";
    controlsRow.appendChild(status);
    const hintPanel = document.createElement("div");
    hintPanel.dataset.role = "code-hint";
    hintPanel.className = "hint-inline hidden";
    const instructions = document.createElement("p");
    instructions.dataset.role = "code-instructions";
    instructions.className = "intro";
    actionBar.append(hintPanel, instructions);
    const answerActions = document.createElement("div");
    answerActions.className = "controls code-mobile-actions";
    codePanel.appendChild(answerActions);
    const actionButtons = Array.from(controlsRow.querySelectorAll('[data-role="code-hint-btn"], [data-role="code-check"], [data-role="code-reroll"], [data-role="code-show-failing-case"], [data-role="code-status"]'));
    const placeActions = () => {
        const mobile = isMobileViewport();
        const target = mobile ? answerActions : controlsRow;
        actionButtons.forEach((button) => target.appendChild(button));
        if (mobile)
            answerActions.appendChild(hintPanel);
        else
            actionBar.insertBefore(hintPanel, instructions);
    };
    placeActions();
    window.matchMedia("(max-width: 900px)").addEventListener("change", placeActions);
    return collectCodeLessonElements(section);
}
export { ensureCodeLessonLayout };
