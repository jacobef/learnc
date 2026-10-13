import { renderStatePanel } from "./shared-workspace-dom.js";
import { bindBtnRefPulse, clearNode, createStepper, flashStatus, setPartsContent, } from "./shared-core.js";
import { bindCodeEditorTabKey, ensureCodeSurfaceElements, updateCodeSurface, } from "./shared-code-editor-surface.js";
import { ensureCodeLessonLayout } from "./shared-code-lesson-layout.js";
import { diagnosticDecorations, diagnosticMessageText, offsetDiagnosticLines, } from "./shared-diagnostics.js";
import { challengeFileName, challengePrefix, challengeSuffix, normalizeChallengeInput, runChallengeProgram, } from "./shared-code-challenge-input.js";
import { codeRuntimeIssue, renderCodeDiagnostic, diagnosticRuntimeContextText, offsetCodeRuntimeIssue, renderCodeRuntimeIssue, } from "./shared-code-runtime-issues.js";
import { boxValueMatchesSpec } from "./shared-c-value-semantics.js";
import { sameCType } from "./shared-core-utils.js";
import { createButtonTokenReplacer, createHintPresenter, createLevelProgressController, invalidTemplateConfig, nextLessonLabel, resetLevelAfterConfirmation, } from "./shared-lesson-runtime.js";
// The boxes that make up a named object: the object itself and, for an array, its elements.
function objectBoxes(state, name) {
    return state.filter((box) => box.name === name || box.arrayRoot === name);
}
function lineCount(text) {
    return text === "" ? 0 : text.replace(/\n$/, "").split("\n").length;
}
function createCodeOutputChallengeTemplate(config) {
    const { inputs = [], outputs = [], checkStdout = false, testInputs = [[]], solve, instructions = "", startCode = "", textareaMinLines = 5, allowNewLines = true, hints = null, next = null, nextLabel, } = config;
    const startInput = config.startInput ?? testInputs[0] ?? [];
    const endLabel = nextLessonLabel({
        next,
        fallback: "Next Program",
        override: nextLabel,
    });
    const ensureIdentifier = (name, label) => {
        const trimmed = String(name || "").trim();
        if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(trimmed)) {
            invalidTemplateConfig(`${label} must be a valid C identifier.`);
        }
        return trimmed;
    };
    if (!outputs.length && !checkStdout) {
        invalidTemplateConfig("A challenge must check outputs, stdout, or both.");
    }
    if (config.files?.length && (config.implicitMain ?? !config.main)) {
        invalidTemplateConfig("A challenge with several files can't use implicit main.");
    }
    const inputSpecs = inputs.map((spec, index) => ({
        name: ensureIdentifier(spec?.name || "", `Input name ${index + 1}`),
        type: String(spec?.type || "").trim(),
        allocated: !!spec?.allocated,
    }));
    const outputSpecs = outputs.map((spec, index) => ({
        name: ensureIdentifier(spec?.name || "", `Output name ${index + 1}`),
        type: String(spec?.type || "").trim(),
    }));
    const shape = {
        inputs: inputSpecs,
        prelude: config.prelude,
        main: config.main,
        implicitMain: config.implicitMain,
        files: config.files,
        fileName: config.fileName,
    };
    const fileName = challengeFileName(shape);
    if (!testInputs.length ||
        testInputs.some((row) => !Array.isArray(row) ||
            row.length !== inputSpecs.length ||
            row.some((value) => typeof value !== "string"))) {
        invalidTemplateConfig(`testInputs must be a non-empty list of string arrays of length ${inputSpecs.length}.`);
    }
    if (startInput.length !== inputSpecs.length ||
        startInput.some((value) => typeof value !== "string")) {
        invalidTemplateConfig(`startInput must be a string array of length ${inputSpecs.length}.`);
    }
    const solveCode = String(solve || "").replace(/\r\n/g, "\n");
    if (!solveCode.trim()) {
        invalidTemplateConfig("solve must be a non-empty C code string.");
    }
    const { instructionsEl, lockedLineNumbers, lockedInputLine, lockedSuffixNumbers, lockedSuffixLines, editor, lineNumbers, stage, status, diagnosticEl, hintPanel, hintBtn, checkBtn, levelResetBtn, rerollBtn, showFailBtn, prevBtn, nextBtn, codeRoot, } = ensureCodeLessonLayout({
        textareaMinLines,
        lockedInput: true,
        lockedSuffix: !!shape.main,
        readOnlyFiles: shape.files,
        fileName,
    });
    const { highlightEl, measureEl } = ensureCodeSurfaceElements(editor);
    bindBtnRefPulse(codeRoot || document);
    function normalizeInputLiteral(value, spec, label) {
        try {
            return normalizeChallengeInput(value, spec);
        }
        catch (error) {
            return invalidTemplateConfig(`${label} ${error instanceof Error ? error.message : String(error)}`);
        }
    }
    const targetOutputNameSet = new Set(outputSpecs.map((spec) => spec.name));
    function prefixLineCount(testCase) {
        return lineCount(challengePrefix(shape, testCase.inputLiterals));
    }
    function expectedFromSolve(inputLiterals, label) {
        const analyzed = runChallengeProgram(shape, inputLiterals, solveCode);
        if (analyzed.kind !== "ok" || analyzed.executionLimit) {
            return invalidTemplateConfig(analyzed.kind === "compile"
                ? `solve does not compile for ${label}: ${diagnosticMessageText(analyzed.diagnostic)}`
                : `solve does not finish cleanly for ${label}.`);
        }
        const expectedOutputs = outputSpecs.map((outputSpec) => {
            const boxes = objectBoxes(analyzed.state, outputSpec.name);
            const root = boxes.find((box) => box.name === outputSpec.name);
            if (!root)
                invalidTemplateConfig(`solve must create ${outputSpec.name} for ${label}.`);
            if (!sameCType(root.type, outputSpec.type)) {
                invalidTemplateConfig(`solve must produce ${outputSpec.name} with type ${outputSpec.type} for ${label}.`);
            }
            // An array's own box has no value; everything else must have one.
            const valued = boxes.filter((box) => box.arrayRoot || !box.arrayShape?.length);
            if (valued.some((box) => box.value === "")) {
                invalidTemplateConfig(`solve leaves part of ${outputSpec.name} without a value for ${label}.`);
            }
            return boxes;
        });
        return { expectedOutputs, expectedStdout: analyzed.stdout };
    }
    function createChallengeCaseForInputRow(rawInputRow, label) {
        const normalizedInputs = inputSpecs.map((inputSpec, index) => normalizeInputLiteral(rawInputRow[index], inputSpec, `${label} input ${inputSpec.name}`));
        const inputLiterals = normalizedInputs.map((item) => item.literal);
        return {
            inputValues: normalizedInputs.map((item) => item.runtime),
            inputLiterals,
            ...expectedFromSolve(inputLiterals, label),
        };
    }
    const testCases = testInputs.map((row, index) => createChallengeCaseForInputRow(row, `testInputs[${index}]`));
    function copyCase(testCase) {
        return {
            inputValues: testCase.inputValues.slice(),
            inputLiterals: testCase.inputLiterals.slice(),
            expectedOutputs: testCase.expectedOutputs.map((boxes) => boxes.slice()),
            expectedStdout: testCase.expectedStdout,
        };
    }
    function caseInputKey(testCase) {
        return testCase.inputLiterals.join("\u0000");
    }
    function pickDifferentTestCase(currentCase) {
        const currentKey = caseInputKey(currentCase);
        const candidates = testCases.filter((testCase) => caseInputKey(testCase) !== currentKey);
        if (!candidates.length)
            return copyCase(currentCase);
        return copyCase(candidates[Math.floor(Math.random() * candidates.length)]);
    }
    function restoreCase(inputLiterals) {
        if (!Array.isArray(inputLiterals) || inputLiterals.length !== inputSpecs.length) {
            return null;
        }
        try {
            return createChallengeCaseForInputRow(inputLiterals, "saved progress");
        }
        catch {
            return null;
        }
    }
    const progress = createLevelProgressController(isDefaultProgress);
    const defaultText = normalizeUserCodeText(startCode);
    const defaultVisibleCase = createChallengeCaseForInputRow(startInput, "startInput");
    const restoredProgress = progress.restore();
    const restoredVisibleCase = restoreCase(restoredProgress?.visibleCaseInputLiterals);
    const restoredPendingCase = restoreCase(restoredProgress?.pendingFailingCaseInputLiterals);
    const restoredHadRunChecks = restoredProgress?.hasRunChecks === true;
    const state = {
        text: typeof restoredProgress?.text === "string"
            ? normalizeUserCodeText(restoredProgress.text)
            : defaultText,
        pass: restoredProgress?.pass === true,
        visibleCase: restoredVisibleCase || copyCase(defaultVisibleCase),
        testCases,
        lastReport: null,
        pendingFailingCase: restoredPendingCase,
        showFullShownOutput: restoredProgress?.showFullShownOutput === true,
    };
    let pager = null;
    function normalizeUserCodeText(text) {
        const normalized = text.replace(/\r\n/g, "\n");
        if (allowNewLines)
            return normalized;
        return normalized.replace(/\n/g, " ");
    }
    function adjustSelectionForCarriageReturns(text, pos) {
        let removed = 0;
        for (let i = 0; i < pos && i < text.length; i++) {
            if (text[i] === "\r")
                removed += 1;
        }
        return Math.max(0, pos - removed);
    }
    function getUserText() {
        const raw = editor ? editor.value : state.text || "";
        return normalizeUserCodeText(raw);
    }
    function getEditorText() {
        const { inputLiterals } = state.visibleCase;
        return challengePrefix(shape, inputLiterals) + getUserText() + challengeSuffix(shape, inputLiterals);
    }
    function runCase(testCase, body = getUserText()) {
        return runChallengeProgram(shape, testCase.inputLiterals, body);
    }
    if (editor) {
        const lines = Math.max(1, Number(textareaMinLines));
        editor.style.minHeight = `calc(var(--code-line-height) * ${lines} + 16px)`;
        editor.value = state.text || "";
    }
    function getUserRawLines() {
        const raw = editor ? editor.value : state.text || "";
        return raw.split(/\r?\n/);
    }
    function syncEditorLinkedScroll() {
        if (!editor)
            return;
        if (lineNumbers)
            lineNumbers.scrollTop = editor.scrollTop;
    }
    let visibleProgramFeedback = {
        diagnostic: null,
        runtimeIssue: null,
    };
    let visibleCaseResult = null;
    // Feedback in the learner's line numbering. Problems in locked code or other
    // files keep their location but don't decorate the editor.
    function getVisibleProgramFeedback(result) {
        const offset = -prefixLineCount(state.visibleCase);
        if (result.kind !== "ok") {
            const diagnostic = result.diagnostic;
            const inLearnerFile = !diagnostic.file || diagnostic.file === fileName;
            return {
                diagnostic: inLearnerFile ? offsetDiagnosticLines(diagnostic, offset) : diagnostic,
                runtimeIssue: null,
            };
        }
        const issue = codeRuntimeIssue(result);
        return {
            diagnostic: null,
            runtimeIssue: issue ? offsetCodeRuntimeIssue(issue, offset) : null,
        };
    }
    function diagnosticInEditor(diagnostic) {
        if (!diagnostic || (diagnostic.file && diagnostic.file !== fileName))
            return null;
        const line = diagnostic.range.startLine;
        return line >= 0 && line < getUserRawLines().length ? diagnostic : null;
    }
    function updateLineGutters(diagnostic, runtimeIssue) {
        const lines = getUserRawLines();
        const editorDiagnostic = diagnosticInEditor(diagnostic);
        const lineNumberClasses = new Map();
        const problemLine = editorDiagnostic?.range.startLine ?? runtimeIssue?.range.startLine;
        if (problemLine !== undefined && problemLine >= 0 && problemLine < lines.length) {
            lineNumberClasses.set(problemLine, ["has-error"]);
        }
        const prefixLines = prefixLineCount(state.visibleCase);
        updateCodeSurface({
            editor,
            lineNumbers,
            highlightEl,
            measureEl,
            lines,
            lineNumberStart: prefixLines + 1,
            decorations: diagnosticDecorations(editorDiagnostic, lines),
            lineNumberClasses,
        });
        syncEditorLinkedScroll();
        if (diagnostic) {
            const inLearnerFile = !diagnostic.file || diagnostic.file === fileName;
            renderCodeDiagnostic(diagnosticEl, editor, diagnostic, {
                displayedLineOffset: inLearnerFile ? prefixLines : 0,
                activeFile: shape.files?.length ? fileName : undefined,
            });
        }
        else {
            renderCodeRuntimeIssue(diagnosticEl, editor, runtimeIssue, prefixLines);
        }
    }
    function evaluateProgramBehavior(analyzed) {
        if (analyzed.kind !== "ok")
            return { kind: analyzed.kind };
        const runtimeIssue = codeRuntimeIssue(analyzed);
        if (runtimeIssue) {
            return { kind: runtimeIssue.kind === "execution-limit" ? "step-limit" : "blocked-input" };
        }
        const outputBoxes = [];
        for (const outputSpec of outputSpecs) {
            const boxes = objectBoxes(analyzed.state, outputSpec.name);
            const root = boxes.find((box) => box.name === outputSpec.name);
            if (!root)
                return { kind: "missing-output" };
            if (!sameCType(root.type, outputSpec.type))
                return { kind: "wrong-output-type" };
            outputBoxes.push(boxes);
        }
        return { kind: "ok", outputBoxes, stdout: analyzed.stdout };
    }
    // Whether actual has every expected box, with matching values.
    function outputMatches(actual, expected) {
        return expected.every((expectedBox) => {
            const actualBox = actual.find((box) => box.name === expectedBox.name);
            return !!actualBox && boxValueMatchesSpec(actualBox, expectedBox).ok;
        });
    }
    function behavesLikeProgramOnTestInputs(program) {
        const candidate = program.replace(/\r\n/g, "\n");
        if (!candidate.trim())
            return false;
        const userBody = getUserText();
        return testCases.every((testCase) => {
            const user = evaluateProgramBehavior(runCase(testCase, userBody));
            const other = evaluateProgramBehavior(runCase(testCase, candidate));
            if (user.kind !== other.kind)
                return false;
            if (user.kind !== "ok" || other.kind !== "ok")
                return true;
            return ((!checkStdout || user.stdout === other.stdout) &&
                user.outputBoxes.every((boxes, index) => outputMatches(boxes, other.outputBoxes[index])));
        });
    }
    function evaluateCase(testCase, analyzed = runCase(testCase)) {
        const fallbackExpected = testCase.expectedOutputs[0]?.[0] ?? null;
        const failure = (kind, resultState, extra = {}) => ({
            ok: false,
            kind,
            state: resultState,
            stdout: analyzed.kind === "ok" ? analyzed.stdout : "",
            outputBox: null,
            expected: fallbackExpected,
            failingOutput: outputSpecs[0] ?? null,
            ...extra,
        });
        if (analyzed.kind !== "ok") {
            return failure(analyzed.kind, analyzed.diagnostic.runtimeContext?.state ?? null);
        }
        const runtimeIssue = codeRuntimeIssue(analyzed);
        if (runtimeIssue) {
            return runtimeIssue.kind === "execution-limit"
                ? failure("step-limit", analyzed.state)
                : failure("blocked-input", analyzed.blocked?.state ?? analyzed.state);
        }
        const finalState = analyzed.state;
        for (let index = 0; index < outputSpecs.length; index += 1) {
            const outputSpec = outputSpecs[index];
            const expected = testCase.expectedOutputs[index];
            const boxes = objectBoxes(finalState, outputSpec.name);
            const actual = boxes.find((box) => box.name === outputSpec.name) ?? null;
            const details = { outputBox: actual, expected: expected[0] ?? null, failingOutput: outputSpec };
            if (!actual)
                return failure("missing-output", finalState, details);
            if (!sameCType(actual.type, outputSpec.type))
                return failure("wrong-output-type", finalState, details);
            if (!outputMatches(boxes, expected))
                return failure("wrong-output-value", finalState, details);
        }
        if (checkStdout && analyzed.stdout !== testCase.expectedStdout) {
            return failure("wrong-stdout", finalState, { failingOutput: null });
        }
        return {
            ok: true,
            kind: "ok",
            state: finalState,
            stdout: analyzed.stdout,
            outputBox: finalState.find((box) => box.name === outputSpecs[0]?.name) ?? null,
            expected: fallbackExpected,
            failingOutput: null,
        };
    }
    function runAllCases() {
        const items = state.testCases.map((testCase, index) => ({
            index,
            testCase,
            result: evaluateCase(testCase),
        }));
        const firstFailure = items.find((item) => !item.result.ok) || null;
        return { pass: !firstFailure, items, firstFailure };
    }
    function expectedStateBoxesForCase(testCase) {
        return testCase.expectedOutputs.flat().map((box) => ({
            ...box,
            address: "<i>(any)</i>",
            aliases: [],
        }));
    }
    // How an expected output reads in a hint: a value, or an array's values.
    function expectedValueText(boxes) {
        const elements = boxes.filter((box) => box.arrayRoot);
        if (!elements.length)
            return `$v{${boxes[0]?.value ?? ""}}`;
        return `$v{\\{ ${elements.map((box) => box.value).join(", ")} \\}}`;
    }
    function progressSnapshot() {
        return {
            text: getUserText(),
            pass: state.pass,
            visibleCaseInputLiterals: state.visibleCase.inputLiterals.slice(),
            pendingFailingCaseInputLiterals: state.pendingFailingCase
                ? state.pendingFailingCase.inputLiterals.slice()
                : null,
            showFullShownOutput: state.showFullShownOutput,
            hasRunChecks: !!state.lastReport,
        };
    }
    function isDefaultProgress(snapshot) {
        return (snapshot.text === defaultText &&
            !snapshot.pass &&
            caseInputKey({ inputLiterals: snapshot.visibleCaseInputLiterals }) ===
                caseInputKey(defaultVisibleCase) &&
            snapshot.pendingFailingCaseInputLiterals == null &&
            !snapshot.showFullShownOutput &&
            !snapshot.hasRunChecks);
    }
    function persistProgress() {
        progress.save(progressSnapshot());
    }
    function renderStdoutPanel(title, text, emptyMessage) {
        const panel = document.createElement("div");
        panel.className = "state-panel";
        const heading = document.createElement("h3");
        heading.className = "panel-title state-heading";
        heading.textContent = title;
        panel.appendChild(heading);
        const pre = document.createElement("pre");
        pre.className = "program-output-text";
        if (text === null) {
            pre.classList.add("muted");
            pre.textContent = emptyMessage;
        }
        else {
            pre.textContent = text;
            const cursor = document.createElement("span");
            cursor.className = "program-output-cursor";
            cursor.setAttribute("aria-hidden", "true");
            pre.appendChild(cursor);
        }
        panel.appendChild(pre);
        return panel;
    }
    function renderStage(currentResult) {
        if (!stage)
            return;
        clearNode(stage);
        const ran = currentResult.kind !== "compile" &&
            currentResult.kind !== "ub" &&
            currentResult.kind !== "step-limit" &&
            currentResult.kind !== "blocked-input";
        const notRunMessage = currentResult.kind === "compile"
            ? "(fix the error above to run the program)"
            : currentResult.kind === "step-limit"
                ? "(the program was stopped before it finished)"
                : currentResult.kind === "blocked-input"
                    ? "(the program began waiting for input)"
                    : "(the program stopped because of undefined behavior)";
        if (outputSpecs.length) {
            const group = document.createElement("div");
            group.className = "state-group two-col";
            const fullShownState = currentResult.state || [];
            const isTarget = (box) => targetOutputNameSet.has(box.name) || targetOutputNameSet.has(box.arrayRoot ?? "");
            const hasExtraShownVars = ran && fullShownState.some((box) => !isTarget(box) && !box.anonymous);
            const shownBoxes = !ran || state.showFullShownOutput
                ? fullShownState
                : fullShownState.filter(isTarget);
            const shownTitle = currentResult.kind === "ub" && currentResult.state
                ? "Last recorded state before undefined behavior"
                : currentResult.kind === "step-limit"
                    ? "State before the repeating section"
                    : currentResult.kind === "blocked-input"
                        ? "Program state when it began waiting"
                        : currentResult.kind === "compile"
                            ? "Program did not run"
                            : "Your code's result";
            const shownEmptyMessage = !ran
                ? notRunMessage
                : state.showFullShownOutput
                    ? "(no variables)"
                    : outputSpecs.length === 1
                        ? `(missing variable ${outputSpecs[0].name})`
                        : "(missing one or more result variables)";
            const shownControls = (() => {
                if (!hasExtraShownVars)
                    return null;
                const toggle = document.createElement("button");
                toggle.type = "button";
                toggle.className = "state-panel-toggle";
                toggle.textContent = state.showFullShownOutput
                    ? outputSpecs.length === 1
                        ? "Show only result variable"
                        : "Show only result variables"
                    : "Show full state";
                toggle.addEventListener("click", () => {
                    state.showFullShownOutput = !state.showFullShownOutput;
                    renderStage(currentResult);
                    persistProgress();
                });
                return toggle;
            })();
            group.appendChild(renderStatePanel(shownTitle, shownBoxes, {
                emptyMessage: shownEmptyMessage,
                controls: shownControls,
            }));
            group.appendChild(renderStatePanel("Expected result", expectedStateBoxesForCase(state.visibleCase)));
            stage.appendChild(group);
        }
        if (checkStdout) {
            const group = document.createElement("div");
            group.className = "state-group two-col";
            group.append(renderStdoutPanel("Your program's output", ran ? currentResult.stdout : null, notRunMessage), renderStdoutPanel("Expected output", state.visibleCase.expectedStdout, ""));
            stage.appendChild(group);
        }
    }
    function buttonReplacements() {
        const backLabel = (prevBtn?.textContent || "Back ◀").trim();
        return [
            ["$checkButton", "$b{Check}"],
            ["$newInputButton", "$b{New input}"],
            ["$showFailingCaseButton", "$b{Show failing case}"],
            ["$runLineButton", "$b{Run line}"],
            ["$backButton", `$b{${backLabel}}`],
        ];
    }
    const applyButtonTokens = createButtonTokenReplacer(buttonReplacements);
    function setStatus(text, cls = "muted") {
        if (!status)
            return;
        status.textContent = text;
        status.className = cls;
    }
    function renderLockedLines(numbersEl, linesEl, text, firstLine) {
        const lines = text === "" ? [] : text.replace(/\n$/, "").split("\n");
        linesEl?.closest(".code-locked-row")?.classList.toggle("hidden", lines.length === 0);
        if (linesEl)
            linesEl.textContent = lines.join("\n");
        if (!numbersEl)
            return;
        clearNode(numbersEl);
        lines.forEach((_, index) => {
            const num = document.createElement("div");
            num.className = "code-line-number";
            num.textContent = String(firstLine + index);
            numbersEl.appendChild(num);
        });
    }
    function updateLockedLines() {
        const { inputLiterals } = state.visibleCase;
        const prefix = challengePrefix(shape, inputLiterals);
        renderLockedLines(lockedLineNumbers, lockedInputLine, prefix, 1);
        renderLockedLines(lockedSuffixNumbers, lockedSuffixLines, challengeSuffix(shape, inputLiterals), lineCount(prefix) + getUserRawLines().length + 1);
    }
    function updateInstructions() {
        if (state.pass) {
            setPartsContent(instructionsEl, "Challenge solved!");
            return;
        }
        setPartsContent(instructionsEl, applyButtonTokens(instructions || null));
    }
    const { hide: hideHint, show: showHint } = createHintPresenter(hintPanel, applyButtonTokens);
    function defaultHint(current, report) {
        if (current.kind === "compile") {
            const diagnostic = visibleProgramFeedback.diagnostic;
            return diagnostic
                ? diagnosticMessageText(diagnostic)
                : "The shown case does not compile yet. Fix syntax errors first.";
        }
        if (current.kind === "ub") {
            const diagnostic = visibleProgramFeedback.diagnostic;
            if (diagnostic) {
                return [
                    diagnosticMessageText(diagnostic),
                    diagnosticRuntimeContextText(diagnostic),
                ]
                    .filter(Boolean)
                    .join(" ");
            }
            return "The shown case has undefined behavior.";
        }
        if (current.kind === "step-limit") {
            return "The shown case did not finish. Check whether every loop's condition eventually becomes zero.";
        }
        if (current.kind === "blocked-input") {
            return "The shown case is waiting for input, but code-writing lessons do not provide interactive input. Remove the input operation or initialize the value directly.";
        }
        if (current.kind === "missing-output") {
            return current.failingOutput
                ? `Create a variable named $n{${current.failingOutput.name}}.`
                : "Create the result variables.";
        }
        if (current.kind === "wrong-output-type") {
            return current.failingOutput
                ? `$n{${current.failingOutput.name}} should have type $t{${current.failingOutput.type}}.`
                : "One result variable has the wrong type.";
        }
        if (current.kind === "wrong-output-value") {
            const index = outputSpecs.findIndex((spec) => spec.name === current.failingOutput?.name);
            const expected = state.visibleCase.expectedOutputs[index];
            if (current.failingOutput && expected) {
                return `For the shown input, $n{${current.failingOutput.name}} should be ${expectedValueText(expected)}.`;
            }
            return "For the shown input, one result variable has the wrong value.";
        }
        if (current.kind === "wrong-stdout") {
            return "For the shown input, your program's output doesn't match the expected output.";
        }
        if (report && !report.pass) {
            return "The shown input works, but at least one other test input fails. Click $showFailingCaseButton to see one that fails.";
        }
        return "Looks good. Press $checkButton.";
    }
    function render() {
        const analyzed = runCase(state.visibleCase);
        const currentResult = evaluateCase(state.visibleCase, analyzed);
        visibleCaseResult = currentResult;
        visibleProgramFeedback = getVisibleProgramFeedback(analyzed);
        renderStage(currentResult);
        updateLockedLines();
        updateInstructions();
        updateLineGutters(visibleProgramFeedback.diagnostic, visibleProgramFeedback.runtimeIssue);
        if (state.pass) {
            setStatus("correct", "ok");
        }
        else if (state.lastReport && !state.lastReport.pass) {
            setStatus("incorrect", "err");
        }
        else {
            setStatus("", "muted");
        }
        const editable = !state.pass;
        if (checkBtn)
            checkBtn.classList.toggle("hidden", !editable);
        if (hintBtn)
            hintBtn.classList.toggle("hidden", !editable);
        const visibleCaseKey = caseInputKey(state.visibleCase);
        const hasAlternateInput = state.testCases.some((testCase) => caseInputKey(testCase) !== visibleCaseKey);
        if (rerollBtn) {
            rerollBtn.classList.toggle("hidden", !editable || !hasAlternateInput);
        }
        if (showFailBtn) {
            showFailBtn.classList.toggle("hidden", !editable || !state.pendingFailingCase || !!(currentResult && !currentResult.ok));
        }
        if (editor)
            editor.readOnly = !editable;
        if (!editable)
            editor?.classList.add("readonly");
        nextBtn?.classList.remove("hidden");
        pager?.update();
        if (levelResetBtn) {
            levelResetBtn.disabled = isDefaultProgress(progressSnapshot());
        }
        persistProgress();
    }
    function buildHintContext(currentResult, report) {
        return {
            text: getEditorText(),
            inputs: inputSpecs.map((inputSpec, index) => ({
                ...inputSpec,
                value: state.visibleCase.inputValues[index],
            })),
            outputs: outputSpecs.map((outputSpec) => ({ ...outputSpec })),
            currentCase: state.visibleCase,
            currentResult,
            report,
            behavesLike: behavesLikeProgramOnTestInputs,
        };
    }
    if (editor) {
        bindCodeEditorTabKey(editor);
        if (!allowNewLines) {
            editor.addEventListener("keydown", (event) => {
                if (event.key === "Enter")
                    event.preventDefault();
            });
        }
        editor.addEventListener("input", () => {
            const raw = editor.value;
            const normalized = normalizeUserCodeText(raw);
            if (normalized !== raw) {
                const start = adjustSelectionForCarriageReturns(raw, editor.selectionStart);
                const end = adjustSelectionForCarriageReturns(raw, editor.selectionEnd);
                editor.value = normalized;
                const clampedStart = Math.min(normalized.length, start);
                const clampedEnd = Math.min(normalized.length, end);
                editor.setSelectionRange(clampedStart, clampedEnd);
            }
            state.text = editor.value;
            state.lastReport = null;
            state.pendingFailingCase = null;
            render();
        });
        editor.addEventListener("scroll", syncEditorLinkedScroll);
        window.addEventListener("resize", () => {
            updateLineGutters(visibleProgramFeedback.diagnostic, visibleProgramFeedback.runtimeIssue);
        });
        if (typeof ResizeObserver !== "undefined") {
            const ro = new ResizeObserver(() => {
                updateLineGutters(visibleProgramFeedback.diagnostic, visibleProgramFeedback.runtimeIssue);
            });
            ro.observe(editor);
        }
    }
    if (levelResetBtn) {
        levelResetBtn.addEventListener("click", () => {
            resetLevelAfterConfirmation(progress);
        });
    }
    if (rerollBtn) {
        rerollBtn.addEventListener("click", () => {
            if (state.pass)
                return;
            hideHint();
            state.visibleCase = pickDifferentTestCase(state.visibleCase);
            state.lastReport = null;
            state.pendingFailingCase = null;
            setStatus("", "muted");
            render();
        });
    }
    if (hintBtn) {
        hintBtn.addEventListener("click", () => {
            hideHint();
            const currentResult = visibleCaseResult ?? evaluateCase(state.visibleCase);
            if (currentResult.kind === "compile" ||
                currentResult.kind === "ub" ||
                currentResult.kind === "step-limit" ||
                currentResult.kind === "blocked-input") {
                state.pendingFailingCase = null;
                showHint(defaultHint(currentResult));
                return;
            }
            const report = runAllCases();
            if (currentResult.ok && !report.pass) {
                const failingCase = report.firstFailure?.testCase || null;
                state.pendingFailingCase = failingCase ? copyCase(failingCase) : null;
            }
            else {
                state.pendingFailingCase = null;
            }
            render();
            let parts = null;
            if (typeof hints === "function") {
                parts = hints(buildHintContext(currentResult, report));
            }
            else {
                parts = hints;
            }
            if (!parts || (Array.isArray(parts) && parts.length === 0)) {
                parts = defaultHint(currentResult, report);
            }
            showHint(parts);
        });
    }
    if (showFailBtn) {
        showFailBtn.addEventListener("click", () => {
            if (state.pass || !state.pendingFailingCase)
                return;
            hideHint();
            state.visibleCase = copyCase(state.pendingFailingCase);
            state.pendingFailingCase = null;
            render();
        });
    }
    if (checkBtn) {
        checkBtn.addEventListener("click", () => {
            hideHint();
            const report = runAllCases();
            state.lastReport = report;
            if (!report.pass) {
                const failingCase = report.firstFailure?.testCase || null;
                state.pendingFailingCase = failingCase ? copyCase(failingCase) : null;
                render();
                const failureKind = report.firstFailure?.result.kind;
                const failureStatus = failureKind === "compile"
                    ? "fix the error shown above"
                    : failureKind === "ub"
                        ? "fix the undefined behavior shown above"
                        : failureKind === "step-limit"
                            ? "program did not finish"
                            : failureKind === "blocked-input"
                                ? "program is waiting for input"
                                : "incorrect";
                setStatus(failureStatus, "err");
                flashStatus(status);
                return;
            }
            state.pass = true;
            state.pendingFailingCase = null;
            if (editor)
                editor.readOnly = true;
            checkBtn?.classList.add("hidden");
            hintBtn?.classList.add("hidden");
            rerollBtn?.classList.add("hidden");
            showFailBtn?.classList.add("hidden");
            pager?.pulseNext();
            pager?.update();
            render();
            setStatus("correct", "ok");
            flashStatus(status);
        });
    }
    if (restoredHadRunChecks && !state.pass) {
        state.lastReport = runAllCases();
        if (state.lastReport.pass) {
            state.pendingFailingCase = null;
        }
        else if (!state.pendingFailingCase) {
            const failingCase = state.lastReport.firstFailure?.testCase || null;
            state.pendingFailingCase = failingCase ? copyCase(failingCase) : null;
        }
    }
    pager = createStepper({
        root: codeRoot || editor?.closest(".panel") || document.body,
        lines: 0,
        nextPage: next || null,
        endLabel,
        getBoundary: () => 0,
        setBoundary: () => { },
        onAfterChange: render,
        isStepLocked: () => !state.pass,
    });
    pager.update();
    render();
}
export { createCodeOutputChallengeTemplate };
