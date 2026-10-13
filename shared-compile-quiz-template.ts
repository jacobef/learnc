import {
  bindBtnRefPulse,
  clearNode,
  createStepper,
  ensurePanelizedMain,
  flashStatus,
  queryRole,
  renderCodePane,
  setPartsContent,
  syncDocumentTitleFromNav,
} from "./shared-core.js";
import type { Parts, Stepper } from "./shared-core.js";
import { runCFiles, runCProgram } from "./shared-c-interpreter.js";
import { diagnosticMessageText } from "./shared-diagnostics.js";
import {
  createButtonTokenReplacer,
  createHintPresenter,
  createLevelProgressController,
  invalidTemplateConfig,
  nextLessonLabel,
  resetLevelAfterConfirmation,
} from "./shared-lesson-runtime.js";

// What happens when a snippet is compiled and run.
type QuizOutcome = "compiles" | "error" | "ub";

interface QuizStep {
  code: string;
  instructions?: Parts;
  // Shown once the step is answered (immediately, if it isn't a question).
  explanation?: Parts;
  hint?: Parts;
  // Whether the learner has to say what happens. Defaults to true.
  question?: boolean;
}

interface CompileQuizConfig {
  steps: QuizStep[];
  initialInstructions?: string;
  // False when the snippets define main themselves.
  implicitMain?: boolean;
  // Offer "Undefined behavior" as an answer, for programs that compile but have UB.
  allowUndefinedAnswer?: boolean;
  next: string | null;
  nextLabel?: string;
}

interface CompileQuizProgress {
  boundary: number;
  answered: number[];
}

interface QuizResult {
  outcome: QuizOutcome;
  message: string;
  line: number | null;
}

const ANSWER_LABELS: Record<QuizOutcome, string> = {
  compiles: "Compiles",
  error: "Doesn't compile",
  ub: "Undefined behavior",
};

function ensureCompileQuizLayout() {
  const resolvedTitle = syncDocumentTitleFromNav();
  const main = ensurePanelizedMain(resolvedTitle);
  const section = document.createElement("section");
  section.dataset.role = "quiz-root";
  section.className = "panel-shell";
  const actionBar = document.createElement("div");
  actionBar.className = "controls-bar";
  const controlsMain = document.createElement("div");
  controlsMain.className = "controls-main panel panel-controls";
  const controlsRow = document.createElement("div");
  controlsRow.className = "controls-row controls-left";
  const prevBtn = document.createElement("button");
  prevBtn.dataset.stepper = "prev";
  prevBtn.textContent = "Back ◀";
  const nextBtn = document.createElement("button");
  nextBtn.dataset.stepper = "next";
  nextBtn.textContent = "Next ▶";
  const spacer = document.createElement("span");
  spacer.className = "controls-spacer";
  const resetBtn = document.createElement("button");
  resetBtn.textContent = "Reset level";
  controlsRow.append(prevBtn, nextBtn, spacer, resetBtn);
  controlsMain.appendChild(controlsRow);
  const hintPanel = document.createElement("div");
  hintPanel.className = "hint-inline hidden";
  const instructionsEl = document.createElement("p");
  instructionsEl.className = "intro";
  actionBar.append(controlsMain, hintPanel, instructionsEl);

  const codePanel = document.createElement("div");
  codePanel.className = "panel code-panel-shell quiz-code-panel";
  const codeTitle = document.createElement("div");
  codeTitle.className = "panel-title code-title";
  codeTitle.textContent = "Code";
  const codeEl = document.createElement("div");
  codeEl.className = "codepane panel-body";
  codePanel.append(codeTitle, codeEl);

  const answerPanel = document.createElement("div");
  answerPanel.className = "panel quiz-answer-panel";
  const answerTitle = document.createElement("div");
  answerTitle.className = "panel-title";
  answerTitle.textContent = "What happens?";
  const choices = document.createElement("div");
  choices.className = "quiz-choices";
  const statusRow = document.createElement("div");
  statusRow.className = "quiz-status-row";
  const hintBtn = document.createElement("button");
  hintBtn.type = "button";
  hintBtn.className = "hint-link";
  hintBtn.textContent = "Hint";
  const statusEl = document.createElement("span");
  statusEl.className = "muted";
  statusRow.append(hintBtn, statusEl);
  const resultEl = document.createElement("div");
  resultEl.className = "quiz-result hidden";
  answerPanel.append(answerTitle, choices, statusRow, resultEl);

  section.append(actionBar, codePanel, answerPanel);
  main.appendChild(section);
  return {
    section, prevBtn, nextBtn, resetBtn, hintPanel, hintBtn, instructionsEl,
    codeEl, answerPanel, choices, statusEl, resultEl,
  };
}

function createCompileQuizTemplate(config: CompileQuizConfig): void {
  const {
    steps,
    initialInstructions = "",
    implicitMain = true,
    allowUndefinedAnswer = false,
    next,
    nextLabel,
  } = config;
  if (!steps.length) invalidTemplateConfig("A compile quiz needs at least one step.");
  if (queryRole("quiz-root")) return;
  const ui = ensureCompileQuizLayout();
  bindBtnRefPulse(ui.section);

  const results: QuizResult[] = steps.map((step, index) => {
    const source = step.code.endsWith("\n") ? step.code : `${step.code}\n`;
    const run = implicitMain
      ? runCProgram(source)
      : runCFiles([{ path: "program.c", source }], undefined, "", false);
    if (run.kind === "ok" && run.executionLimit) {
      invalidTemplateConfig(`Quiz step ${index} does not finish.`);
    }
    if (run.kind === "ub" && !allowUndefinedAnswer && step.question !== false) {
      invalidTemplateConfig(`Quiz step ${index} has undefined behavior, which isn't an answer here.`);
    }
    return run.kind === "ok"
      ? { outcome: "compiles", message: "", line: null }
      : {
          outcome: run.kind === "compile" ? "error" : "ub",
          message: diagnosticMessageText(run.diagnostic),
          line: run.diagnostic.range.startLine,
        };
  });
  const isQuestion = (index: number) => steps[index]?.question !== false;

  const progress = createLevelProgressController<CompileQuizProgress>(
    (snapshot) => snapshot.boundary === 0 && snapshot.answered.length === 0,
  );
  const restored = progress.restore();
  let boundary = Math.max(0, Math.min(steps.length - 1, restored?.boundary ?? 0));
  const answered = new Set<number>(
    (restored?.answered ?? []).filter((index) => index >= 0 && index < steps.length),
  );
  let pager: Stepper | null = null;

  const applyButtonTokens = createButtonTokenReplacer(() => [
    ...(Object.entries(ANSWER_LABELS).map(([outcome, label]) => [
      `$${outcome}Button`,
      `$b{${label}}`,
    ] as const)),
  ]);
  const { hide: hideHint, show: showHint } = createHintPresenter(ui.hintPanel, applyButtonTokens);

  function persist() {
    progress.save({ boundary, answered: [...answered] });
  }

  function revealed(index: number): boolean {
    return !isQuestion(index) || answered.has(index);
  }

  function renderResult(index: number) {
    const result = results[index]!;
    const step = steps[index]!;
    clearNode(ui.resultEl);
    ui.resultEl.classList.toggle("hidden", !revealed(index));
    if (!revealed(index)) return;
    // A step that isn't a question just explains something.
    if (!isQuestion(index)) {
      if (step.explanation) setPartsContent(ui.resultEl, applyButtonTokens(step.explanation));
      return;
    }
    const heading = document.createElement("div");
    heading.className = `quiz-result-title quiz-result-${result.outcome}`;
    heading.textContent =
      result.outcome === "error"
        ? `Doesn't compile${result.line == null ? "" : ` (line ${result.line + 1})`}`
        : result.outcome === "ub"
          ? `Undefined behavior${result.line == null ? "" : ` (line ${result.line + 1})`}`
          : "Compiles";
    ui.resultEl.appendChild(heading);
    if (step.explanation) {
      const explanation = document.createElement("div");
      setPartsContent(explanation, applyButtonTokens(step.explanation));
      ui.resultEl.appendChild(explanation);
    }
    if (result.message) {
      const message = document.createElement("div");
      message.className = "quiz-compiler-message";
      message.textContent = `${result.outcome === "error" ? "The compiler says" : "The interpreter says"}: ${result.message}`;
      ui.resultEl.appendChild(message);
    }
  }

  function render() {
    const step = steps[boundary]!;
    const result = results[boundary]!;
    const instructions = [
      boundary === 0 ? initialInstructions : "",
      ...(Array.isArray(step.instructions) ? step.instructions : [step.instructions ?? ""]),
    ].filter(Boolean).join("\n");
    setPartsContent(ui.instructionsEl, applyButtonTokens(instructions || null));
    const lines = step.code.replace(/\n$/, "").split("\n");
    renderCodePane(ui.codeEl, lines, lines.length, { hideBoundary: true });
    if (revealed(boundary) && result.line != null) {
      ui.codeEl.querySelectorAll(".line")[result.line]?.classList.add("quiz-problem-line");
    }
    clearNode(ui.choices);
    const outcomes: QuizOutcome[] = allowUndefinedAnswer ? ["compiles", "error", "ub"] : ["compiles", "error"];
    for (const outcome of outcomes) {
      const button = document.createElement("button");
      button.type = "button";
      button.textContent = ANSWER_LABELS[outcome];
      button.disabled = revealed(boundary);
      if (revealed(boundary) && outcome === result.outcome) button.classList.add("is-active");
      button.addEventListener("click", () => answer(outcome));
      ui.choices.appendChild(button);
    }
    ui.choices.classList.toggle("hidden", !isQuestion(boundary));
    ui.answerPanel.querySelector(".panel-title")!.textContent = isQuestion(boundary) ? "What happens?" : "";
    ui.hintBtn.classList.toggle("hidden", revealed(boundary) || !step.hint);
    ui.statusEl.textContent = answered.has(boundary) ? "correct" : "";
    ui.statusEl.className = answered.has(boundary) ? "ok" : "muted";
    renderResult(boundary);
    hideHint();
    pager?.update();
    persist();
  }

  function answer(outcome: QuizOutcome) {
    if (revealed(boundary)) return;
    if (outcome !== results[boundary]!.outcome) {
      ui.statusEl.textContent = "incorrect";
      ui.statusEl.className = "err";
      flashStatus(ui.statusEl);
      return;
    }
    answered.add(boundary);
    render();
    flashStatus(ui.statusEl);
    pager?.pulseNext();
  }

  ui.hintBtn.addEventListener("click", () => {
    const hint = steps[boundary]?.hint;
    if (hint) showHint(hint);
  });
  ui.resetBtn.addEventListener("click", () => resetLevelAfterConfirmation(progress));

  const endLabel = nextLessonLabel({ next, fallback: "Next Page", override: nextLabel });
  pager = createStepper({
    root: ui.section,
    lines: Math.max(0, steps.length - 1),
    nextPage: next,
    getBoundary: () => boundary,
    setBoundary: (value) => {
      boundary = Math.max(0, Math.min(steps.length - 1, Math.floor(value)));
    },
    isStepLocked: (at) => !revealed(at),
    onAfterChange: () => render(),
    getNextLabel: (_boundary, _total, atEnd) => (atEnd ? endLabel : "Next"),
    endLabel,
  });
  render();
}

export { createCompileQuizTemplate };
