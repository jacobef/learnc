import {
  appendStateObjects,
  applyTextTokenReplacements,
  bindBtnRefPulse,
  clearNode,
  createStepper,
  disableBoxEditing,
  ensurePanelizedMain,
  findArrayObjectBoxesForResult,
  flashStatus,
  getNavLabelForHref,
  readBoxState,
  queryRole,
  setPartsContent,
  syncDocumentTitleFromNav,
  vbox,
} from "./shared-core.js";
import type { BoxState, CTypeInfo, Parts, Stepper } from "./shared-core.js";
import {
  boxValueMatchesSpec,
  createSyntheticAddressBase,
  evaluateCExpression,
  normalizeBoxValueForContext,
  runCProgram,
} from "./shared-c-interpreter.js";
import {
  clearLevelProgress,
  currentLevelId,
  maybeRestoreLevelProgress,
  writeLevelProgress,
} from "./shared-progress.js";

type AnswerMode = "selected" | "entered";
type ExpressionHint = (ctx: ExpressionHintContext) => Parts | null | undefined;

interface ExpressionStep {
  expression: string;
  setup?: string;
  instructions?: Parts;
  hints?: Parts | ExpressionHint;
  editable?: boolean;
  fixValueCategory?: boolean;
}

interface ExpressionTemplateConfig {
  steps: ExpressionStep[];
  initialInstructions?: string;
  next: string | null;
  isLast?: boolean;
  workspace?: ExpressionWorkspaceConfig;
}

interface ExpressionWorkspaceConfig {
  alwaysShowExprResult?: boolean;
}

interface ExpressionTemplateState {
  boundary: number;
  passes: Record<number, boolean>;
  selections: Record<number, string | null>;
  entered: Record<number, BoxState | null>;
  modes: Record<number, AnswerMode>;
  showIntro: boolean;
}

interface ExpressionTemplateElements {
  instructionsEl: HTMLElement | null;
  continueBtn: HTMLButtonElement | null;
  levelResetBtn: HTMLButtonElement | null;
  sectionEl: HTMLElement | null;
  expressionEl: HTMLElement | null;
  answerPanel: HTMLElement | null;
  answerSlot: HTMLElement | null;
  answerResult: HTMLElement | null;
  toggleWrap: HTMLElement | null;
  hintPanel: HTMLElement | null;
  hintBtn: HTMLButtonElement | null;
  useSelectedBtn: HTMLButtonElement | null;
  useEnteredBtn: HTMLButtonElement | null;
  checkBtn: HTMLButtonElement | null;
  statusEl: HTMLElement | null;
  stageEl: HTMLElement | null;
  statePanel: HTMLElement | null;
}

interface ExpressionHintContext {
  step: ExpressionStep;
  mode: AnswerMode;
  selectedName: string | null;
  selectedBox: BoxState | null;
  enteredBox: BoxState | null;
  expected:
    | { kind: "error"; message: string }
    | { kind: "lvalue"; address: string }
    | { kind: "rvalue"; type: string; value: string; typeInfo: CTypeInfo };
  ok: boolean;
  hasState: boolean;
}

interface ExpressionTemplateProgress {
  boundary: number;
  passes: Record<number, boolean>;
  selections: Record<number, string | null>;
  entered: Record<number, BoxState | null>;
  modes: Record<number, AnswerMode>;
  showIntro: boolean;
}

function collectExpressionElements(
  root: ParentNode = document,
): ExpressionTemplateElements {
  const role = <T extends Element>(name: string) => queryRole<T>(name, root);
  return {
    instructionsEl: role<HTMLElement>("expr-instructions"),
    continueBtn: role<HTMLButtonElement>("expr-continue"),
    levelResetBtn: role<HTMLButtonElement>("expr-reset-level"),
    sectionEl: role<HTMLElement>("expr-section"),
    expressionEl: role<HTMLElement>("expr-expression"),
    answerPanel: role<HTMLElement>("expr-answer-panel"),
    answerSlot: role<HTMLElement>("expr-answer-slot"),
    answerResult: role<HTMLElement>("expr-answer-result"),
    toggleWrap: role<HTMLElement>("expr-answer-toggle"),
    hintPanel: role<HTMLElement>("expr-hint"),
    hintBtn: role<HTMLButtonElement>("expr-hint-btn"),
    useSelectedBtn: role<HTMLButtonElement>("expr-use-selected"),
    useEnteredBtn: role<HTMLButtonElement>("expr-use-entered"),
    checkBtn: role<HTMLButtonElement>("expr-check"),
    statusEl: role<HTMLElement>("expr-status"),
    stageEl: role<HTMLElement>("expr-stage"),
    statePanel: role<HTMLElement>("expr-state-panel"),
  };
}

function ensureExpressionLayout(): ExpressionTemplateElements {
  const resolvedTitle = syncDocumentTitleFromNav();
  const existing = queryRole<HTMLElement>("expr-expression");
  if (existing) return collectExpressionElements();

  const main = ensurePanelizedMain(resolvedTitle);

  const instructionsEl = document.createElement("p");
  instructionsEl.dataset.role = "expr-instructions";
  instructionsEl.className = "intro";

  const continueBtn = document.createElement("button");
  continueBtn.type = "button";
  continueBtn.dataset.role = "expr-continue";
  continueBtn.textContent = "Continue";

  const section = document.createElement("section");
  section.dataset.role = "expr-section";
  section.classList.add("panel-shell");
  const actionBar = document.createElement("div");
  actionBar.className = "controls-bar controls-bar-expr";
  const controlsMain = document.createElement("div");
  controlsMain.className = "controls-main panel panel-controls";
  const controlsRow = document.createElement("div");
  controlsRow.className = "controls-row controls-left";
  controlsMain.appendChild(controlsRow);
  actionBar.appendChild(controlsMain);
  section.appendChild(actionBar);
  const stack = document.createElement("div");
  stack.className = "expr-eval-stack panel-stack";
  section.appendChild(stack);
  main.appendChild(section);

  const exprPanel = document.createElement("div");
  exprPanel.className = "panel quiz-expression-panel panel-scroll";
  const exprTitle = document.createElement("div");
  exprTitle.className = "panel-title";
  exprTitle.textContent = "Expression";
  const exprDisplay = document.createElement("div");
  exprDisplay.dataset.role = "expr-expression";
  exprDisplay.className = "expr-display expr-quiz-display panel-body";
  exprPanel.appendChild(exprTitle);
  exprPanel.appendChild(exprDisplay);

  const leftCol = document.createElement("div");
  leftCol.className = "expr-left-col panel-col";

  const row = document.createElement("div");
  row.className = "expr-eval-row panel-row";

  const answerPanel = document.createElement("div");
  answerPanel.dataset.role = "expr-answer-panel";
  answerPanel.className = "panel expr-answer-panel panel-scroll";
  const answerTitle = document.createElement("div");
  answerTitle.className = "panel-title";
  answerTitle.textContent = "Expression result";
  const answerStack = document.createElement("div");
  answerStack.className = "expr-answer-stack panel-body";
  const answerResult = document.createElement("div");
  answerResult.dataset.role = "expr-answer-result";
  answerResult.className = "expr-answer-result hidden";
  const answerSlot = document.createElement("div");
  answerSlot.dataset.role = "expr-answer-slot";
  answerSlot.className = "expr-answer-slot";

  const toggleWrap = document.createElement("div");
  toggleWrap.dataset.role = "expr-answer-toggle";
  toggleWrap.className = "expr-answer-toggle";
  const useSelectedBtn = document.createElement("button");
  useSelectedBtn.type = "button";
  useSelectedBtn.dataset.role = "expr-use-selected";
  useSelectedBtn.textContent = "From state";
  const useEnteredBtn = document.createElement("button");
  useEnteredBtn.type = "button";
  useEnteredBtn.dataset.role = "expr-use-entered";
  useEnteredBtn.textContent = "Type box";
  toggleWrap.appendChild(useSelectedBtn);
  toggleWrap.appendChild(useEnteredBtn);

  const prevBtn = document.createElement("button");
  prevBtn.dataset.stepper = "prev";
  prevBtn.textContent = "Back ◀";
  const checkBtn = document.createElement("button");
  checkBtn.dataset.role = "expr-check";
  checkBtn.textContent = "Check";
  const nextBtn = document.createElement("button");
  nextBtn.dataset.stepper = "next";
  nextBtn.textContent = "Next ▶";
  const levelResetBtn = document.createElement("button");
  levelResetBtn.type = "button";
  levelResetBtn.dataset.role = "expr-reset-level";
  levelResetBtn.textContent = "Reset level";
  const controlsSpacer = document.createElement("span");
  controlsSpacer.className = "controls-spacer";
  controlsSpacer.setAttribute("aria-hidden", "true");
  const statusEl = document.createElement("span");
  statusEl.dataset.role = "expr-status";
  statusEl.className = "muted expr-check-inline";
  const hintBtn = document.createElement("button");
  hintBtn.type = "button";
  hintBtn.dataset.role = "expr-hint-btn";
  hintBtn.textContent = "Hint";
  controlsRow.appendChild(prevBtn);
  controlsRow.appendChild(nextBtn);
  controlsRow.appendChild(controlsSpacer);
  controlsRow.appendChild(levelResetBtn);
  controlsRow.appendChild(continueBtn);
  controlsRow.appendChild(hintBtn);
  controlsRow.appendChild(checkBtn);
  controlsRow.appendChild(statusEl);

  answerStack.appendChild(answerResult);
  answerStack.appendChild(answerSlot);
  answerStack.appendChild(toggleWrap);
  const hintPanel = document.createElement("div");
  hintPanel.dataset.role = "expr-hint";
  hintPanel.className = "hint-inline hidden";
  actionBar.appendChild(hintPanel);
  actionBar.appendChild(instructionsEl);
  answerPanel.appendChild(answerTitle);
  answerPanel.appendChild(answerStack);

  const statePanel = document.createElement("div");
  statePanel.className = "panel program-state-panel panel-scroll";
  statePanel.dataset.role = "expr-state-panel";
  const stateTitle = document.createElement("div");
  stateTitle.className = "panel-title";
  stateTitle.textContent = "Program state";
  const stageEl = document.createElement("div");
  stageEl.dataset.role = "expr-stage";
  stageEl.className = "grid panel-body";
  statePanel.appendChild(stateTitle);
  statePanel.appendChild(stageEl);

  leftCol.appendChild(exprPanel);
  leftCol.appendChild(answerPanel);
  row.appendChild(leftCol);
  row.appendChild(statePanel);
  stack.appendChild(row);

  return collectExpressionElements();
}

function createExpressionEvalTemplate(config: ExpressionTemplateConfig): void {
  const {
    steps = [],
    initialInstructions = "",
    next = null,
    isLast = false,
    workspace = {},
  } = config;
  const alwaysShowExprResult = workspace.alwaysShowExprResult !== false;
  const endLabel = (() => {
    if (isLast) return "Finish";
    const label = getNavLabelForHref(next);
    return label ? `Next: ${label}` : "Next Page";
  })();
  const failConfig = (message: string): never => {
    alert(message);
    throw new Error(message);
  };
  if (!Array.isArray(steps) || !steps.length) {
    failConfig("Expression template requires at least one step.");
  }
  const normalizedSteps = steps.map((step) => ({
    ...step,
    editable: step.editable === true,
  }));
  const usedAddressBases = new Set<number>();
  const addressBases = normalizedSteps.map(() => {
    let addressBase = createSyntheticAddressBase();
    while (usedAddressBases.has(addressBase)) {
      addressBase = createSyntheticAddressBase();
    }
    usedAddressBases.add(addressBase);
    return addressBase;
  });
  const addressBaseByStep = new Map<ExpressionStep, number>(
    normalizedSteps.map((step, index) => [step, addressBases[index]!] as const),
  );

  const {
    instructionsEl,
    continueBtn,
    levelResetBtn,
    sectionEl,
    expressionEl,
    answerPanel,
    answerSlot,
    answerResult,
    toggleWrap,
    hintPanel,
    hintBtn,
    useSelectedBtn,
    useEnteredBtn,
    checkBtn,
    statusEl,
    stageEl,
    statePanel,
  } = ensureExpressionLayout();

  bindBtnRefPulse(sectionEl || document);

  let pager: Stepper | null = null;
  let selectedName: string | null = null;
  let activeMode: AnswerMode = "selected";

  function stepFor(index: number): ExpressionStep {
    return normalizedSteps[
      Math.max(0, Math.min(normalizedSteps.length - 1, index))
    ]!;
  }

  function addressBaseForStep(step: ExpressionStep): number {
    return addressBaseByStep.get(step) ?? addressBases[0]!;
  }

  function clampBoundary(value: number): number {
    return Math.max(0, Math.min(normalizedSteps.length - 1, Math.floor(value)));
  }

  function sanitizedRecord<T>(
    value: Record<number, T> | null | undefined,
  ): Record<number, T> {
    if (!value || typeof value !== "object") return {};
    const entries = Object.entries(value).filter(([key]) => /^\d+$/.test(key));
    return Object.fromEntries(entries) as Record<number, T>;
  }

  function sanitizedPassedSelections(
    value: Record<number, string | null> | null | undefined,
    passes: Record<number, boolean>,
  ): Record<number, string | null> {
    if (!value || typeof value !== "object") return {};
    const entries = Object.entries(value).filter(
      ([key, selected]) => passes[Number(key)] === true && typeof selected === "string",
    );
    return Object.fromEntries(entries) as Record<number, string | null>;
  }

  function defaultModeForStep(index: number): AnswerMode {
    const step = stepFor(index);
    if (step.editable && step.fixValueCategory) {
      const evaluated = evaluatedAnswer(step);
      if ("result" in evaluated && evaluated.result) {
        return evaluated.result.kind === "lvalue" ? "selected" : "entered";
      }
    }
    return "selected";
  }

  const levelId = currentLevelId();
  const defaultShowIntro = !!initialInstructions;
  const restoredProgress =
    maybeRestoreLevelProgress<ExpressionTemplateProgress>(levelId);
  const restoredPasses = sanitizedRecord<boolean>(restoredProgress?.passes);
  const state: ExpressionTemplateState = {
    boundary: clampBoundary(restoredProgress?.boundary ?? 0),
    passes: restoredPasses,
    selections: sanitizedPassedSelections(
      restoredProgress?.selections,
      restoredPasses,
    ),
    entered: sanitizedRecord<BoxState | null>(restoredProgress?.entered),
    modes: sanitizedRecord<AnswerMode>(restoredProgress?.modes),
    showIntro:
      typeof restoredProgress?.showIntro === "boolean"
        ? restoredProgress.showIntro
        : defaultShowIntro,
  };

  function setStatus(text: string, ok: boolean, silent: boolean = false) {
    if (!statusEl) return;
    statusEl.textContent = text;
    statusEl.className = `${ok ? "ok" : "err"} expr-check-inline`;
    if (!silent) flashStatus(statusEl);
  }

  function clearStatus() {
    if (!statusEl) return;
    statusEl.textContent = "";
    statusEl.className = "muted expr-check-inline";
  }

  function setControlVisible(
    control: HTMLElement | null,
    visible: boolean,
  ) {
    if (!control) return;
    control.classList.toggle("hidden", !visible);
  }

  function showHint(text: Parts | null | undefined) {
    if (!hintPanel) return;
    renderHint(text);
    hintPanel.classList.remove("hidden");
    flashStatus(hintPanel);
  }

  function hideHint() {
    if (!hintPanel) return;
    hintPanel.classList.add("hidden");
  }

  function visibleButtonLabel(
    button: HTMLButtonElement | null,
    fallback: string,
  ): string {
    return (button?.textContent || fallback).trim();
  }

  function nextButtonEl(): HTMLButtonElement | null {
    const button = document.querySelector('button[data-stepper="next"]');
    return button instanceof HTMLButtonElement ? button : null;
  }

  function previousButtonEl(): HTMLButtonElement | null {
    const button = document.querySelector('button[data-stepper="prev"]');
    return button instanceof HTMLButtonElement ? button : null;
  }

  function buttonReplacements() {
    const nextLabel = visibleButtonLabel(nextButtonEl(), "Next ▶");
    const previousLabel = visibleButtonLabel(previousButtonEl(), "Back ◀");
    return [
      ["$nextButton", `$b{${nextLabel}}`],
      ["$runLineButton", `$b{${nextLabel}}`],
      ["$backButton", `$b{${previousLabel}}`],
      ["$checkButton", "$b{Check}"],
      ["$hintButton", "$b{Hint}"],
      ["$resetButton", "$b{Reset level}"],
      ["$continueButton", "$b{Continue}"],
      ["$fromStateButton", "$b{From state}"],
      ["$typeBoxButton", "$b{Type box}"],
    ] as const;
  }

  function applyButtonTokens(parts: Parts | null): Parts | null {
    return applyTextTokenReplacements(parts, buttonReplacements()) as
      | Parts
      | null;
  }

  function renderInstructions(parts: Parts | null) {
    setPartsContent(instructionsEl, applyButtonTokens(parts));
  }

  function renderHint(parts: Parts | null | undefined) {
    setPartsContent(hintPanel, applyButtonTokens(parts ?? null));
  }

  function setActiveMode(mode: AnswerMode, editable: boolean) {
    activeMode = mode;
    if (useSelectedBtn) {
      useSelectedBtn.classList.toggle("is-active", mode === "selected");
      useSelectedBtn.setAttribute(
        "aria-pressed",
        mode === "selected" ? "true" : "false",
      );
      useSelectedBtn.disabled = !editable;
    }
    if (useEnteredBtn) {
      useEnteredBtn.classList.toggle("is-active", mode === "entered");
      useEnteredBtn.setAttribute(
        "aria-pressed",
        mode === "entered" ? "true" : "false",
      );
      useEnteredBtn.disabled = !editable;
    }
  }

  function updateSelectionHighlight(name: string | null, enabled: boolean) {
    if (!stageEl) return;
    stageEl.querySelectorAll(".vbox").forEach((node) => {
      const el = node as HTMLElement;
      const isSelected = enabled && !!name && el.dataset.name === name;
      el.classList.toggle("quiz-selected", isSelected);
    });
  }

  function sourceForStep(step: ExpressionStep): string {
    const setup = String(step.setup ?? "").trim();
    return setup ? `${setup}\n0;` : "0;";
  }

  function runtimeForStep(step: ExpressionStep):
    | {
        source: string;
        state: BoxState[];
        eventIndex: number;
        addressBase: number;
      }
    | { error: true; kind: "compile" | "ub" } {
    const source = sourceForStep(step);
    const addressBase = addressBaseForStep(step);
    const result = runCProgram(source, addressBase);
    if (result.kind !== "ok") return { error: true, kind: result.kind };
    const trace = result.trace || [];
    if (!trace.length) {
      return {
        source,
        state: result.state,
        eventIndex: 0,
        addressBase,
      };
    }
    const eventIndex = trace.length - 1;
    return {
      source,
      state: trace[eventIndex]?.state ?? result.state,
      eventIndex,
      addressBase,
    };
  }

  function boxesForStep(step: ExpressionStep): BoxState[] {
    const runtime = runtimeForStep(step);
    return "error" in runtime ? [] : runtime.state;
  }

  function selectedBox(step: ExpressionStep): BoxState | null {
    if (!selectedName) return null;
    const boxes = boxesForStep(step);
    return boxes.find((box) => box.name === selectedName) || null;
  }

  function getFixedMode(step: ExpressionStep): AnswerMode | null {
    if (!step.editable || !step.fixValueCategory) return null;
    const evaluated = evaluatedAnswer(step);
    if ("result" in evaluated && evaluated.result) {
      return evaluated.result.kind === "lvalue" ? "selected" : "entered";
    }
    return null;
  }

  function renderSelectedPreview(step: ExpressionStep) {
    if (!answerSlot) return;
    clearNode(answerSlot);
    const box = selectedBox(step);
    if (!box) {
      const msg = document.createElement("div");
      msg.className = "expr-answer-empty muted";
      msg.textContent = "No box selected yet.";
      answerSlot.appendChild(msg);
      return;
    }
    const node = vbox({
      address: box.address ?? undefined,
      type: box.type,
      value: box.value,
      displayValue: box.displayValue,
      exactValue: box.exactValue,
      typeInfo: box.typeInfo,
      aliases: box.aliases ?? [],
      name: box.name,
      editable: false,
    });
    if ((box.value ?? "") === "")
      node.querySelector(".value")?.classList.add("placeholder", "muted");
    answerSlot.appendChild(node);
  }

  function renderEnteredBox(saved: BoxState | null, editable: boolean) {
    if (!answerSlot) return;
    clearNode(answerSlot);
    const node = vbox({
      address: "—",
      type: saved?.type ?? "",
      value: saved?.rawValue ?? saved?.value ?? "",
      name: "",
      editable,
      allowNameEdit: false,
      allowTypeEdit: editable,
      showDoubleExact: saved?.showDoubleExact ?? false,
    });
    node.classList.add("no-name", "no-addr");
    answerSlot.appendChild(node);
  }

  function setSelected(name: string | null) {
    selectedName = name;
    const step = stepFor(state.boundary);
    updateSelectionHighlight(name, activeMode === "selected");
    if (activeMode === "selected") renderSelectedPreview(step);
  }

  function renderProgramState(
    step: ExpressionStep,
    editable: boolean,
    fixedMode: AnswerMode | null,
  ) {
    if (!stageEl) return;
    clearNode(stageEl);
    const boxes = boxesForStep(step);
    const disableHover = !editable || fixedMode === "entered";
    boxes.forEach((box) => {
      const node = vbox({
        address: box.address ?? undefined,
        type: box.type,
        value: box.value,
        displayValue: box.displayValue,
        exactValue: box.exactValue,
        typeInfo: box.typeInfo,
        aliases: box.aliases ?? [],
        name: box.name,
        editable: false,
      });
      if ((box.value ?? "") === "")
        node.querySelector(".value")?.classList.add("placeholder", "muted");
      node.dataset.name = box.name;
      node.classList.add("quiz-selectable");
      if (editable) {
        if (!disableHover) {
          node.addEventListener("click", () => setSelected(box.name));
        }
      } else {
        node.classList.add("quiz-static");
      }
      if (disableHover) node.classList.add("quiz-static");
      stageEl.appendChild(node);
    });
    if (editable) {
      updateSelectionHighlight(selectedName, activeMode === "selected");
    } else {
      const evaluated = evaluatedAnswer(step);
      if (
        "result" in evaluated &&
        evaluated.result &&
        evaluated.result.kind === "lvalue"
      ) {
        const match =
          boxes.find(
            (box) => String(box.address) === evaluated.result.address,
          ) || null;
        updateSelectionHighlight(match?.name ?? null, true);
      }
    }
  }

  function evaluatedAnswer(step: ExpressionStep) {
    const runtime = runtimeForStep(step);
    if ("error" in runtime) return runtime;
    const evaluated = evaluateCExpression(
      runtime.source,
      runtime.eventIndex,
      step.expression,
      runtime.addressBase,
    );
    if (evaluated.kind !== "ok") return { error: true, kind: evaluated.kind } as const;
    return { result: evaluated.result } as const;
  }

  function readEnteredBox(): BoxState | null {
    const node = answerSlot?.querySelector(".vbox") || null;
    return node ? readBoxState(node) : null;
  }

  function progressSnapshot(): ExpressionTemplateProgress {
    const passes = Object.fromEntries(
      Object.entries(state.passes).filter(([, value]) => value === true),
    ) as Record<number, boolean>;
    const selections = Object.fromEntries(
      Object.entries(state.selections).filter(
        ([key, value]) => passes[Number(key)] === true && typeof value === "string",
      ),
    ) as Record<number, string | null>;
    const entered = Object.fromEntries(
      Object.entries(state.entered).filter(([, value]) => value != null),
    ) as Record<number, BoxState | null>;
    const modes = Object.fromEntries(
      Object.entries(state.modes).filter(([key, value]) => {
        const index = Number(key);
        return value === "entered" && defaultModeForStep(index) !== "entered";
      }),
    ) as Record<number, AnswerMode>;
    return {
      boundary: clampBoundary(state.boundary),
      passes,
      selections,
      entered,
      modes,
      showIntro: state.showIntro,
    };
  }

  function persistProgress() {
    const snapshot = progressSnapshot();
    if (isDefaultProgress(snapshot)) {
      clearLevelProgress(levelId);
      return;
    }
    writeLevelProgress(snapshot, levelId);
  }

  function isDefaultProgress(snapshot: ExpressionTemplateProgress): boolean {
    return (
      snapshot.boundary === 0 &&
      Object.keys(snapshot.passes).length === 0 &&
      Object.keys(snapshot.selections).length === 0 &&
      Object.keys(snapshot.entered).length === 0 &&
      Object.keys(snapshot.modes).length === 0 &&
      snapshot.showIntro === defaultShowIntro
    );
  }

  function expectedAnswer(step: ExpressionStep): ExpressionHintContext["expected"] {
    const evaluated = evaluatedAnswer(step);
    if ("error" in evaluated) {
      return { kind: "error", message: errorText(evaluated.kind) };
    }
    if (evaluated.result.kind === "lvalue") {
      return { kind: "lvalue", address: String(evaluated.result.address || "") };
    }
    const type = evaluated.result.type || "int";
    return {
      kind: "rvalue",
      type,
      value: evaluated.result.value ?? "",
      typeInfo: evaluated.result.typeInfo,
    };
  }

  function shouldShowExprResultPane(step: ExpressionStep): boolean {
    if (alwaysShowExprResult) return true;
    const evaluated = evaluatedAnswer(step);
    if ("error" in evaluated) return true;
    if (evaluated.result.kind !== "lvalue") return true;
    if (step.fixValueCategory) return false;
    if (!step.editable) return false;
    return true;
  }

  function buildHintContext(step: ExpressionStep): ExpressionHintContext {
    const expected = expectedAnswer(step);
    const selected = selectedBox(step);
    const entered = readEnteredBox();
    const enteredForContext = entered
      ? normalizeBoxValueForContext(entered)
      : null;
    let ok = false;
    if (expected.kind === "lvalue") {
      ok =
        activeMode === "selected" &&
        !!selected &&
        String(selected.address) === expected.address;
    } else if (expected.kind === "rvalue") {
      const expectedBox: BoxState = {
        name: "",
        type: expected.type,
        value: expected.value,
        typeInfo: expected.typeInfo,
      };
      ok =
        activeMode === "entered" &&
        !!entered &&
        String(entered.type || "").trim() === expected.type &&
        boxValueMatchesSpec(entered, expectedBox).ok;
    }
    return {
      step,
      mode: activeMode,
      selectedName,
      selectedBox: selected,
      enteredBox: enteredForContext,
      expected,
      ok,
      hasState: boxesForStep(step).length > 0,
    };
  }

  function errorText(kind: "compile" | "ub" | undefined) {
    return kind === "ub"
      ? "This expression is undefined."
      : "This expression is not valid.";
  }

  function renderCorrectAnswer(step: ExpressionStep) {
    if (!answerResult) return;
    clearNode(answerResult);
    const evaluated = evaluatedAnswer(step);
    if ("error" in evaluated) {
      const msg = document.createElement("div");
      msg.className = "expr-answer-empty muted";
      msg.textContent = errorText(evaluated.kind);
      answerResult.appendChild(msg);
      return;
    }
    const { result } = evaluated;
    const boxes = boxesForStep(step);
    const arrayBoxes = findArrayObjectBoxesForResult(result, boxes);
    if (arrayBoxes && arrayBoxes.length) {
      const wrap = document.createElement("div");
      appendStateObjects(wrap, arrayBoxes, { editable: false, deletable: false });
      const arrayNode = wrap.querySelector(".arraybox") as HTMLElement | null;
      if (arrayNode) {
        answerResult.appendChild(arrayNode);
        return;
      }
    }
    if (result.kind === "lvalue") {
      const match =
        boxes.find(
          (box) => String(box.address) === result.address,
        ) || null;
      const node = match
        ? vbox({
            address: match.address ?? undefined,
            type: match.type,
            value: match.value,
            displayValue: match.displayValue,
            exactValue: match.exactValue,
            typeInfo: match.typeInfo,
            aliases: match.aliases ?? [],
            name: match.name,
            editable: false,
          })
        : vbox({
            address: result.address || "—",
            type: result.type,
            value: result.value ?? "",
            displayValue: result.displayValue,
            exactValue: result.exactValue,
            typeInfo: result.typeInfo,
            name: "",
            editable: false,
          });
      if (match && (match.value ?? "") === "") {
        node.querySelector(".value")?.classList.add("placeholder", "muted");
      }
      if (!match) node.classList.add("no-name");
      answerResult.appendChild(node);
      return;
    }
    const node = vbox({
      address: "—",
      type: result.type || "int",
      value: result.value ?? "",
      displayValue: result.displayValue,
      exactValue: result.exactValue,
      typeInfo: result.typeInfo,
      name: "",
      editable: false,
    });
    node.classList.add("no-name", "no-addr");
    answerResult.appendChild(node);
  }

  function saveStep(index: number) {
    const step = stepFor(index);
    if (!step.editable) return;
    if (state.passes[index] && activeMode === "selected" && selectedName) {
      state.selections[index] = selectedName;
    } else {
      delete state.selections[index];
    }
    if (activeMode === "entered" && answerSlot) {
      const node = answerSlot.querySelector(".vbox");
      state.entered[index] = node ? readBoxState(node) : null;
    }
    state.modes[index] = activeMode;
  }

  function isStepLocked(index: number) {
    const step = stepFor(index);
    if (!step.editable) return false;
    return !state.passes[index];
  }

  function render() {
    const step = stepFor(state.boundary);
    const hasState = boxesForStep(step).length > 0;
    const showExprResultPane = shouldShowExprResultPane(step);
    pager?.update();
    if (levelResetBtn) {
      levelResetBtn.disabled = isDefaultProgress(progressSnapshot());
    }
    if (state.showIntro) {
      renderInstructions(initialInstructions || null);
      setControlVisible(continueBtn, true);
      setControlVisible(sectionEl, false);
      setControlVisible(hintBtn, false);
      hideHint();
      persistProgress();
      return;
    }
    setControlVisible(continueBtn, false);
    setControlVisible(sectionEl, true);
    setControlVisible(answerPanel, showExprResultPane);
    const instructionText = step.instructions ?? null;
    renderInstructions(instructionText);
    if (expressionEl) expressionEl.textContent = step.expression;
    clearStatus();
    hideHint();

    selectedName = state.selections[state.boundary] ?? null;
    const savedEntered = state.entered[state.boundary] ?? null;
    activeMode = state.modes[state.boundary] ?? "selected";
    const fixedMode = getFixedMode(step);
    if (fixedMode) {
      activeMode = fixedMode;
      state.modes[state.boundary] = fixedMode;
    }

    const passed = !!state.passes[state.boundary];
    if (statePanel) {
      statePanel.classList.toggle("hidden", !hasState);
    }

    if (step.editable) {
      const canEdit = !passed;
      answerResult?.classList.add("hidden");
      answerSlot?.classList.remove("hidden");
      if (fixedMode) toggleWrap?.classList.add("hidden");
      else toggleWrap?.classList.remove("hidden");
      setControlVisible(checkBtn, !passed);
      setControlVisible(hintBtn, !passed && !!step.hints);
      if (passed) setStatus("correct", true, true);
      if (hasState) {
        renderProgramState(step, canEdit, fixedMode);
      }
      if (activeMode === "selected") renderSelectedPreview(step);
      else renderEnteredBox(savedEntered, canEdit);
      setActiveMode(activeMode, canEdit);
    } else {
      answerSlot?.classList.add("hidden");
      toggleWrap?.classList.add("hidden");
      setControlVisible(checkBtn, false);
      setControlVisible(hintBtn, false);
      if (hasState) {
        renderProgramState(step, false, fixedMode);
      }
      answerResult?.classList.remove("hidden");
      renderCorrectAnswer(step);
    }

    if (step.editable && passed) {
      if (checkBtn) checkBtn.disabled = true;
    } else if (checkBtn) {
      checkBtn.disabled = false;
    }
    persistProgress();
  }

  function markCurrentStepPassed() {
    state.passes[state.boundary] = true;
    setControlVisible(checkBtn, false);
    setControlVisible(hintBtn, false);
    setActiveMode(activeMode, false);
    disableBoxEditing(answerSlot);
    pager?.pulseNext();
    pager?.update();
    renderInstructions(stepFor(state.boundary).instructions ?? null);
    persistProgress();
  }

  function checkAnswer() {
    hideHint();
    const step = stepFor(state.boundary);
    if (!step.editable) return;
    const evaluation = evaluatedAnswer(step);
    if ("error" in evaluation) {
      setStatus(errorText(evaluation.kind), false);
      return;
    }
    if (activeMode === "selected") {
      const chosen = selectedBox(step);
      if (!chosen) {
        setStatus("Pick a box first.", false);
        return;
      }
      if (evaluation.result.kind !== "lvalue") {
        setStatus("Incorrect.", false);
        return;
      }
      const ok = String(chosen.address) === evaluation.result.address;
      setStatus(ok ? "correct" : "incorrect", ok);
      if (ok) {
        state.selections[state.boundary] = selectedName;
        markCurrentStepPassed();
      }
      return;
    }

    const entryNode = answerSlot?.querySelector(".vbox") || null;
    const entry = entryNode ? readBoxState(entryNode) : null;
    if (!entry) {
      setStatus("Enter a box first.", false);
      return;
    }
    if (evaluation.result.kind !== "rvalue") {
      setStatus("Incorrect.", false);
      return;
    }
    const expectedType = String(evaluation.result.type || "").trim();
    const expectedValue = evaluation.result.value ?? "";
    const entryType = String(entry.type || "").trim();
    const expectedBox: BoxState = {
      name: "",
      type: expectedType,
      value: expectedValue,
      typeInfo: evaluation.result.typeInfo,
    };
    const match = boxValueMatchesSpec(entry, expectedBox);
    const ok = entryType === expectedType && match.ok;
    setStatus(ok ? "correct" : "incorrect", ok);
    if (ok) {
      entry.value = match.normalized;
      entry.rawValue = match.normalized;
      state.entered[state.boundary] = entry;
      renderEnteredBox(entry, false);
      markCurrentStepPassed();
    }
  }

  if (useSelectedBtn) {
    useSelectedBtn.addEventListener("click", () => {
      const step = stepFor(state.boundary);
      if (!step.editable || step.fixValueCategory) return;
      if (activeMode === "entered") {
        const node = answerSlot?.querySelector(".vbox") || null;
        if (node) state.entered[state.boundary] = readBoxState(node);
      }
      setActiveMode("selected", true);
      state.modes[state.boundary] = "selected";
      updateSelectionHighlight(selectedName, true);
      renderSelectedPreview(stepFor(state.boundary));
      persistProgress();
    });
  }

  if (useEnteredBtn) {
    useEnteredBtn.addEventListener("click", () => {
      const step = stepFor(state.boundary);
      if (!step.editable || step.fixValueCategory) return;
      setActiveMode("entered", true);
      state.modes[state.boundary] = "entered";
      updateSelectionHighlight(selectedName, false);
      renderEnteredBox(state.entered[state.boundary] ?? null, true);
      persistProgress();
    });
  }

  if (answerSlot) {
    answerSlot.addEventListener("input", () => {
      saveStep(state.boundary);
      persistProgress();
    });
    answerSlot.addEventListener("click", () => {
      window.setTimeout(() => {
        saveStep(state.boundary);
        persistProgress();
      }, 0);
    });
  }

  if (checkBtn) {
    checkBtn.addEventListener("click", checkAnswer);
  }

  if (continueBtn) {
    continueBtn.addEventListener("click", () => {
      state.showIntro = false;
      render();
    });
  }

  if (levelResetBtn) {
    levelResetBtn.addEventListener("click", () => {
      const confirmed = window.confirm(
        "Reset your saved progress for this level and start over?",
      );
      if (!confirmed) return;
      clearLevelProgress(levelId);
      window.location.reload();
    });
  }

  if (hintBtn) {
    hintBtn.addEventListener("click", () => {
      const step = stepFor(state.boundary);
      if (!step.editable || !step.hints) return;
      const ctx = buildHintContext(step);
      if (ctx.ok) {
        showHint("Looks good. Press $b{Check}.");
        return;
      }
      const parts =
        typeof step.hints === "function" ? step.hints(ctx) : step.hints;
      if (!parts || (Array.isArray(parts) && parts.length === 0)) {
        showHint(
          "Your program has a problem that isn't covered by a hint, sorry.",
        );
        return;
      }
      showHint(parts);
    });
  }

  pager = createStepper({
    root: document,
    lines: Math.max(0, steps.length - 1),
    nextPage: next,
    getBoundary: () => state.boundary,
    setBoundary: (value) => {
      state.boundary = clampBoundary(value);
    },
    isStepLocked: (at) => isStepLocked(at),
    onBeforeChange: (current) => {
      saveStep(current);
      persistProgress();
    },
    onAfterChange: () => render(),
    getNextLabel: (_boundary, _total, atEnd) => {
      if (atEnd) return endLabel;
      return "Next";
    },
    endLabel,
  });

  render();
}

export { createExpressionEvalTemplate };
