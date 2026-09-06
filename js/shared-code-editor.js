import { appendStateObjects, bindBtnRefPulse, clearNode, createStepper, flashStatus, setPartsContent, } from "./shared-core.js";
import { bindCodeEditorTabKey, ensureCodeSurfaceElements, updateCodeSurface, } from "./shared-code-editor-surface.js";
import { ensureCodeLessonLayout } from "./shared-code-lesson-layout.js";
import { diagnosticDecorations, diagnosticMessageText, renderDiagnosticMessage, } from "./shared-diagnostics.js";
import { runCProgram } from "./shared-c-interpreter.js";
import { appendDiagnosticRuntimeContext, codeRuntimeIssue, diagnosticRuntimeContextText, renderCodeRuntimeIssue, } from "./shared-code-runtime-issues.js";
import { boxValueMatchesSpec } from "./shared-c-value-semantics.js";
import { createButtonTokenReplacer, createHintPresenter, createLevelProgressController, nextLessonLabel, resetLevelAfterConfirmation, } from "./shared-lesson-runtime.js";
function createCodeEditorTemplate(config) {
    const { startCode = "", targetState = [], textareaMinLines, allowNewLines = true, hints = null, instructions = "", next = null, nextLabel, } = config;
    const { instructionsEl, editor, lineNumbers, stage, status, diagnosticEl, hintPanel, hintBtn, checkBtn, levelResetBtn, prevBtn, nextBtn, codeRoot, } = ensureCodeLessonLayout({ textareaMinLines, lockedInput: false });
    const { highlightEl, measureEl } = ensureCodeSurfaceElements(editor);
    bindBtnRefPulse(codeRoot || document);
    const defaultText = normalizeEditorText(startCode);
    const progress = createLevelProgressController(isDefaultProgress);
    const restoredProgress = progress.restore();
    const state = {
        text: typeof restoredProgress?.text === "string"
            ? normalizeEditorText(restoredProgress.text)
            : defaultText,
        pass: restoredProgress?.pass === true,
        allocBase: typeof restoredProgress?.allocBase === "number"
            ? restoredProgress.allocBase
            : null,
    };
    let pager = null;
    const endLabel = nextLessonLabel({
        next,
        fallback: "Next Program",
        override: nextLabel,
    });
    function buttonReplacements() {
        const backLabel = (prevBtn?.textContent || "Back ◀").trim();
        return [
            ["$checkButton", "$b{Check}"],
            ["$resetButton", "$b{Reset}"],
            ["$newVariableButton", "$b{+ New variable}"],
            ["$runLineButton", "$b{Run line}"],
            ["$backButton", `$b{${backLabel}}`],
            ["$showAliasesButton", "$b{Show aliases}"],
        ];
    }
    function normalizeEditorText(text) {
        if (allowNewLines)
            return text;
        return text.replace(/\r\n/g, "\n").replace(/\n/g, " ");
    }
    function progressSnapshot() {
        return {
            text: state.text,
            pass: state.pass,
            allocBase: state.allocBase,
        };
    }
    function isDefaultProgress(snapshot) {
        return snapshot.text === defaultText && !snapshot.pass;
    }
    function persistProgress() {
        progress.save(progressSnapshot());
    }
    function setStatus(text, cls = "muted") {
        if (!status)
            return;
        status.textContent = text;
        status.className = cls;
    }
    function getEditorText() {
        return normalizeEditorText(editor?.value || state.text || "");
    }
    function getEditorLines() {
        return getEditorText().split(/\r?\n/);
    }
    function analyzeUserProgram() {
        const program = runCProgram(getEditorText());
        const runtimeIssue = codeRuntimeIssue(program);
        const diagnostic = program.kind === "ok" ? null : program.diagnostic;
        const outcome = (() => {
            if (program.kind !== "ok") {
                return {
                    kind: program.kind,
                    state: program.diagnostic.runtimeContext?.state ?? null,
                };
            }
            if (runtimeIssue?.kind === "blocked-input") {
                return { kind: runtimeIssue.kind, state: program.blocked?.state ?? null };
            }
            if (runtimeIssue) {
                return { kind: runtimeIssue.kind, state: program.state };
            }
            return { kind: "ok", state: program.state };
        })();
        return { program, outcome, diagnostic, runtimeIssue };
    }
    function applyUserProgram() {
        const analysis = analyzeUserProgram();
        return analysis.outcome.kind === "ok" ? analysis.outcome.state : null;
    }
    function renderDiagnostic(diagnostic) {
        if (!diagnosticEl)
            return;
        if (!diagnostic) {
            diagnosticEl.classList.add("hidden");
            diagnosticEl.textContent = "";
            editor?.removeAttribute("aria-invalid");
            return;
        }
        diagnosticEl.classList.remove("hidden");
        diagnosticEl.replaceChildren();
        const heading = document.createElement("div");
        heading.className = "code-diagnostic-title";
        heading.textContent = `${diagnostic.kind === "ub" ? "Undefined behavior" : "Error"} on line ${diagnostic.range.startLine + 1}, column ${diagnostic.range.startCol + 1}`;
        const message = document.createElement("div");
        message.className = "code-diagnostic-message";
        renderDiagnosticMessage(message, diagnostic);
        diagnosticEl.append(heading, message);
        if (diagnostic.tip) {
            const tip = document.createElement("div");
            tip.className = "code-diagnostic-tip";
            tip.textContent = diagnostic.tip;
            diagnosticEl.appendChild(tip);
        }
        appendDiagnosticRuntimeContext(diagnosticEl, diagnostic);
        editor?.setAttribute("aria-invalid", "true");
    }
    function updateLineGutters(analysis) {
        const { diagnostic, runtimeIssue } = analysis;
        const lines = getEditorLines();
        const lineNumberClasses = new Map();
        const problemLine = diagnostic?.range.startLine ?? runtimeIssue?.range.startLine;
        if (problemLine !== undefined) {
            lineNumberClasses.set(problemLine, ["has-error"]);
        }
        updateCodeSurface({
            editor,
            lineNumbers,
            highlightEl,
            measureEl,
            lines,
            decorations: diagnosticDecorations(diagnostic, lines),
            lineNumberClasses,
        });
        if (diagnostic) {
            renderDiagnostic(diagnostic);
        }
        else {
            renderCodeRuntimeIssue(diagnosticEl, editor, runtimeIssue);
        }
    }
    function isTargetMatch(outcome) {
        if (outcome.kind !== "ok" || !outcome.state)
            return false;
        if (outcome.state.length !== targetState.length)
            return false;
        const byName = new Map(outcome.state.map((box) => [box.name, box]));
        for (const expected of targetState) {
            const actual = byName.get(expected.name);
            if (!actual)
                return false;
            if ((actual.type || "") !== (expected.type || ""))
                return false;
            if (!boxValueMatchesSpec(actual, expected).ok)
                return false;
        }
        return true;
    }
    function evaluate(analysis = analyzeUserProgram()) {
        const outcome = analysis.outcome;
        return { ok: isTargetMatch(outcome), outcome };
    }
    function renderState(title, boxes, emptyMessage = "(no variables)") {
        const wrap = document.createElement("div");
        wrap.className = "state-panel state-panel-scrollable";
        const heading = document.createElement("h3");
        heading.className = "panel-title state-heading";
        heading.textContent = title;
        wrap.appendChild(heading);
        const grid = document.createElement("div");
        grid.className = "grid";
        if (!boxes || !boxes.length) {
            const msg = document.createElement("div");
            msg.className = "muted";
            msg.style.padding = "8px";
            msg.textContent = emptyMessage;
            grid.appendChild(msg);
        }
        else {
            appendStateObjects(grid, boxes, { editable: false, deletable: false });
        }
        const body = document.createElement("div");
        body.className = "state-panel-scroll-body";
        body.appendChild(grid);
        wrap.appendChild(body);
        return wrap;
    }
    function renderStage(outcome) {
        if (!stage)
            return;
        clearNode(stage);
        const group = document.createElement("div");
        group.className = "state-group two-col";
        const stateTitle = outcome.kind === "ub" && outcome.state
            ? "Last recorded state before undefined behavior"
            : outcome.kind === "execution-limit"
                ? "State before the repeating section"
                : outcome.kind === "blocked-input"
                    ? "Program state when it began waiting"
                    : outcome.kind === "compile"
                        ? "Program did not run"
                        : "Your code's final state";
        const emptyMessage = outcome.kind === "compile"
            ? "(fix the error above to run the program)"
            : "(no variables)";
        group.appendChild(renderState(stateTitle, outcome.state, emptyMessage));
        group.appendChild(renderState("Target final state", targetState));
        stage.appendChild(group);
    }
    function partsContext(analysis) {
        return {
            text: getEditorText(),
            targetState,
            diagnostic: analysis.diagnostic,
            applyUserProgram,
        };
    }
    const applyButtonTokens = createButtonTokenReplacer(buttonReplacements);
    const { hide: hideHint, show: showHint } = createHintPresenter(hintPanel, applyButtonTokens);
    function handleHint() {
        if (state.pass)
            return;
        const analysis = analyzeUserProgram();
        const result = evaluate(analysis);
        if (result.ok) {
            showHint("Looks good. Press $checkButton.");
            return;
        }
        if (analysis.runtimeIssue) {
            showHint(`${analysis.runtimeIssue.message} ${analysis.runtimeIssue.tip}`);
            return;
        }
        if (analysis.diagnostic?.kind === "ub") {
            const context = diagnosticRuntimeContextText(analysis.diagnostic);
            showHint([diagnosticMessageText(analysis.diagnostic), context]
                .filter(Boolean)
                .join(" "));
            return;
        }
        if (!hints) {
            showHint(analysis.diagnostic
                ? diagnosticMessageText(analysis.diagnostic)
                : "No hints for this page.");
            return;
        }
        const ctx = partsContext(analysis);
        const parts = typeof hints === "function" ? hints(ctx) : hints;
        if (!parts || (Array.isArray(parts) && parts.length === 0)) {
            showHint("No hint available for this state.");
            return;
        }
        showHint(parts);
    }
    let lastAnalysis = null;
    function render(analysis = analyzeUserProgram()) {
        lastAnalysis = analysis;
        updateLineGutters(analysis);
        renderStage(analysis.outcome);
        if (instructions)
            setPartsContent(instructionsEl, applyButtonTokens(instructions));
        else
            setPartsContent(instructionsEl, []);
        if (!state.pass) {
            setStatus("", "muted");
        }
        nextBtn?.classList.remove("hidden");
        pager?.update();
        if (levelResetBtn) {
            levelResetBtn.disabled = isDefaultProgress(progressSnapshot());
        }
        persistProgress();
    }
    if (editor) {
        bindCodeEditorTabKey(editor);
        editor.value = state.text;
        editor.addEventListener("input", () => {
            state.text = normalizeEditorText(editor.value);
            if (!allowNewLines && editor.value !== state.text)
                editor.value = state.text;
            hideHint();
            if (!state.pass)
                setStatus("", "muted");
            render();
        });
        editor.addEventListener("scroll", () => {
            if (lineNumbers)
                lineNumbers.scrollTop = editor.scrollTop;
        });
        window.addEventListener("resize", () => {
            if (lastAnalysis)
                updateLineGutters(lastAnalysis);
        });
        if (typeof ResizeObserver !== "undefined") {
            const ro = new ResizeObserver(() => {
                if (lastAnalysis)
                    updateLineGutters(lastAnalysis);
            });
            ro.observe(editor);
        }
    }
    if (hintBtn) {
        hintBtn.addEventListener("click", () => {
            hideHint();
            handleHint();
        });
    }
    if (checkBtn) {
        checkBtn.addEventListener("click", () => {
            hideHint();
            const analysis = analyzeUserProgram();
            const result = evaluate(analysis);
            const failureStatus = analysis.runtimeIssue?.status ??
                (analysis.diagnostic?.kind === "ub"
                    ? "fix the undefined behavior shown above"
                    : analysis.diagnostic
                        ? "fix the error shown above"
                        : "incorrect");
            setStatus(result.ok ? "correct" : failureStatus, result.ok ? "ok" : "err");
            flashStatus(status);
            if (!result.ok)
                return;
            state.pass = true;
            if (editor)
                editor.readOnly = true;
            checkBtn.classList.add("hidden");
            hintBtn?.classList.add("hidden");
            pager?.pulseNext();
            render();
        });
    }
    if (levelResetBtn) {
        levelResetBtn.addEventListener("click", () => {
            resetLevelAfterConfirmation(progress);
        });
    }
    pager = createStepper({
        root: codeRoot || editor?.closest(".panel") || document.body,
        lines: 0,
        nextPage: next || null,
        endLabel,
        getBoundary: () => 0,
        setBoundary: () => { },
        onAfterChange: () => render(),
        isStepLocked: () => !state.pass,
    });
    pager.update();
    render();
}
export { createCodeEditorTemplate };
