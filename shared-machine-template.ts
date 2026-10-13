// Lessons that step through a program for the toy machine in shared-toy-machine.ts,
// showing its lights, registers, and memory.

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
import type { Stepper } from "./shared-core.js";
import {
  createButtonTokenReplacer,
  createHintPresenter,
  createLevelProgressController,
  invalidTemplateConfig,
  nextLessonLabel,
  resetLevelAfterConfirmation,
} from "./shared-lesson-runtime.js";
import {
  JUMPS,
  LIGHTS_ADDRESS,
  REGISTERS,
  assemble,
  copyMachine,
  runMachine,
  startState,
  MachineError,
} from "./shared-toy-machine.js";
import type { MachineProgram, MachineStart, MachineState, MachineStep, Register } from "./shared-toy-machine.js";

type PerVisit<T> = T | T[];

interface MachineLessonStep {
  code: string;
  instructions?: PerVisit<string>;
  editable?: PerVisit<boolean>;
  hints?: PerVisit<string>;
}

export interface MachineView {
  // Memory addresses to show, besides the lights' byte.
  memory?: number[];
  // Labels for memory addresses, like "x" for a C variable that lives there.
  memoryNames?: Record<number, string>;
  // Which registers to show. Defaults to all four.
  registers?: Register[];
  // Shows the stack pointer, SP.
  stack?: boolean;
}

interface MachineTemplateConfig {
  steps: MachineLessonStep[];
  initialInstructions?: string;
  start?: MachineStart;
  view?: MachineView;
  // C code the assembly was translated from, with the C line each assembly line
  // (by index) came from.
  c?: { code: string; lines: Record<number, number> };
  next: string | null;
  nextLabel?: string;
}

type StageMode = "none" | "state" | "boundary";

interface Stage {
  step: MachineStep;
  before: MachineState;
  mode: StageMode;
  instructions: string | undefined;
  hint: string | undefined;
  // The boundary execution continues from, between lines.
  afterBoundary: number;
}

interface MachineProgress {
  position: number;
  solved: number;
  entered: Array<{ stage: number; state: MachineState }>;
  boundaries: Array<{ stage: number; boundary: number }>;
}

function resolve<T>(spec: PerVisit<T> | undefined, visit: number): T | undefined {
  return Array.isArray(spec) ? spec[visit] : spec;
}

/** The eight lights, showing the bits of the byte at LIGHTS_ADDRESS. */
export function renderLights(
  value: number,
  onToggle?: (bit: number) => void,
  source = `the byte at address ${LIGHTS_ADDRESS}`,
): HTMLElement {
  const panel = document.createElement("div");
  panel.className = "machine-lights";
  const caption = document.createElement("div");
  caption.className = "machine-lights-caption";
  caption.textContent = `Lights (${source}: ${value}, or 0b${value.toString(2).padStart(8, "0")})`;
  const row = document.createElement("div");
  row.className = "machine-lights-row";
  for (let bit = 7; bit >= 0; bit -= 1) {
    const cell = document.createElement(onToggle ? "button" : "div");
    cell.className = "machine-light";
    if ((value >> bit) & 1) cell.classList.add("is-on");
    const bulb = document.createElement("span");
    bulb.className = "machine-light-bulb";
    const label = document.createElement("span");
    label.className = "machine-light-label";
    label.textContent = `bit ${bit}`;
    const weight = document.createElement("span");
    weight.className = "machine-light-weight";
    weight.textContent = String(1 << bit);
    cell.append(bulb, label, weight);
    if (onToggle && cell instanceof HTMLButtonElement) {
      cell.type = "button";
      cell.setAttribute("aria-pressed", String(!!((value >> bit) & 1)));
      cell.setAttribute("aria-label", `Light ${bit}`);
      cell.addEventListener("click", () => onToggle(bit));
    }
    row.appendChild(cell);
  }
  panel.append(caption, row);
  return panel;
}

/** Registers and memory cells, editable when onChange is given. */
export function renderMachineState(
  state: MachineState,
  view: MachineView,
  onChange?: (state: MachineState) => void,
): HTMLElement {
  const wrap = document.createElement("div");
  wrap.className = "machine-state";
  const toggle = onChange
    ? (bit: number) => {
        const next = copyMachine(state);
        next.memory[LIGHTS_ADDRESS] ^= 1 << bit;
        onChange(next);
      }
    : undefined;
  wrap.appendChild(renderLights(state.memory[LIGHTS_ADDRESS]!, toggle));
  const cell = (label: string, value: number, set?: (value: number) => void, sub?: string) => {
    const box = document.createElement("div");
    box.className = "machine-cell";
    const name = document.createElement("div");
    name.className = "machine-cell-name";
    name.textContent = label;
    const shown = set ? document.createElement("input") : document.createElement("div");
    shown.className = "machine-cell-value";
    if (shown instanceof HTMLInputElement) {
      shown.value = String(value);
      shown.inputMode = "numeric";
      shown.setAttribute("aria-label", label);
      shown.addEventListener("change", () => {
        const parsed = Number(shown.value.trim());
        if (Number.isInteger(parsed) && parsed >= 0 && parsed <= 255) set!(parsed);
        else shown.value = String(value);
      });
    } else {
      shown.textContent = String(value);
    }
    box.append(name, shown);
    if (sub) {
      const note = document.createElement("div");
      note.className = "machine-cell-note";
      note.textContent = sub;
      box.appendChild(note);
    }
    return box;
  };
  const section = (title: string) => {
    const group = document.createElement("div");
    group.className = "machine-group";
    const heading = document.createElement("div");
    heading.className = "machine-group-title";
    heading.textContent = title;
    const cells = document.createElement("div");
    cells.className = "machine-cells";
    group.append(heading, cells);
    wrap.appendChild(group);
    return cells;
  };
  const registers = section("Registers");
  for (const register of view.registers ?? REGISTERS) {
    registers.appendChild(
      cell(register, state.registers[register], onChange && ((value) => {
        const next = copyMachine(state);
        next.registers[register] = value;
        onChange(next);
      })),
    );
  }
  if (view.stack) {
    registers.appendChild(
      cell("SP", state.sp, onChange && ((value) => {
        const next = copyMachine(state);
        next.sp = value;
        onChange(next);
      }), "stack pointer"),
    );
  }
  if (view.memory?.length) {
    const memory = section("Memory");
    for (const address of view.memory) {
      memory.appendChild(
        cell(
          `address ${address}`,
          state.memory[address]!,
          onChange && ((value) => {
            const next = copyMachine(state);
            next.memory[address] = value;
            onChange(next);
          }),
          view.memoryNames?.[address],
        ),
      );
    }
  }
  return wrap;
}

export function sameMachineState(a: MachineState, b: MachineState, view: MachineView): boolean {
  const registers = view.registers ?? REGISTERS;
  return (
    registers.every((register) => a.registers[register] === b.registers[register]) &&
    [...(view.memory ?? []), LIGHTS_ADDRESS].every((address) => a.memory[address] === b.memory[address]) &&
    (!view.stack || a.sp === b.sp)
  );
}

function machineLayout(withC: boolean) {
  const resolvedTitle = syncDocumentTitleFromNav();
  const main = ensurePanelizedMain(resolvedTitle);
  const section = document.createElement("section");
  section.dataset.role = "machine-root";
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

  const codeRow = document.createElement("div");
  codeRow.className = "machine-code-row";
  const panel = (title: string) => {
    const box = document.createElement("div");
    box.className = "panel code-panel-shell";
    const heading = document.createElement("div");
    heading.className = "panel-title code-title";
    heading.textContent = title;
    const body = document.createElement("div");
    body.className = "codepane panel-body";
    box.append(heading, body);
    codeRow.appendChild(box);
    return body;
  };
  const cEl = withC ? panel("C") : null;
  const codeEl = panel("Assembly");

  const statePanel = document.createElement("div");
  statePanel.className = "panel machine-panel";
  const stateTitle = document.createElement("div");
  stateTitle.className = "panel-title";
  stateTitle.textContent = "Machine";
  const actions = document.createElement("div");
  actions.className = "machine-actions hidden";
  const checkBtn = document.createElement("button");
  checkBtn.type = "button";
  checkBtn.textContent = "Check";
  const hintBtn = document.createElement("button");
  hintBtn.type = "button";
  hintBtn.textContent = "Hint";
  const undoBtn = document.createElement("button");
  undoBtn.type = "button";
  undoBtn.textContent = "Reset";
  actions.append(hintBtn, checkBtn, undoBtn);
  const statusEl = document.createElement("div");
  statusEl.className = "machine-status muted";
  const stateEl = document.createElement("div");
  statePanel.append(stateTitle, actions, statusEl, stateEl);

  section.append(actionBar, codeRow, statePanel);
  main.appendChild(section);
  return {
    section, prevBtn, nextBtn, resetBtn, hintPanel, instructionsEl, codeEl, cEl,
    actions, checkBtn, hintBtn, undoBtn, statusEl, stateEl,
  };
}

export function createMachineTemplate(config: MachineTemplateConfig): void {
  const { steps, initialInstructions = "", start, view = {}, c, next, nextLabel } = config;
  if (queryRole("machine-root")) return;
  const source = steps.map((step) => step.code).join("");
  const lines = source.replace(/\n$/, "").split("\n");
  let program: MachineProgram;
  try {
    program = assemble(source);
  } catch (error) {
    return invalidTemplateConfig(`Machine lesson code doesn't assemble: ${error instanceof Error ? error.message : error}`);
  }
  const initial = startState(start);
  const run = runMachine(program, initial);
  if (run.error) invalidTemplateConfig(run.error);

  // Which lesson step each line belongs to.
  const stepOfLine: number[] = [];
  steps.forEach((step, index) => {
    const count = step.code.split("\n").length - 1 || 1;
    for (let i = 0; i < count; i += 1) stepOfLine.push(index);
  });
  const visits = new Map<number, number>();
  const stages: Stage[] = run.steps.map((step, index) => {
    const stepIndex = stepOfLine[step.line] ?? 0;
    const visit = visits.get(stepIndex) ?? 0;
    visits.set(stepIndex, visit + 1);
    const spec = steps[stepIndex]!;
    const editable = !!resolve(spec.editable, visit);
    const nextLine = step.nextIndex === null ? null : program.instructions[step.nextIndex]!.line;
    return {
      step,
      before: index === 0 ? copyMachine(initial) : run.steps[index - 1]!.after,
      mode: !editable ? "none" : JUMPS.has(step.op) ? "boundary" : "state",
      instructions: resolve(spec.instructions, visit),
      hint: resolve(spec.hints, visit),
      afterBoundary: nextLine === null ? step.line + 1 : nextLine,
    };
  });

  const ui = machineLayout(!!c);
  bindBtnRefPulse(ui.section);
  const progress = createLevelProgressController<MachineProgress>(
    (snapshot) => snapshot.position === 0 && snapshot.solved < 0 && !snapshot.entered.length,
  );
  const restored = progress.restore();
  let position = Math.max(0, Math.min(stages.length, restored?.position ?? 0));
  let solved = restored?.solved ?? -1;
  const entered = new Map((restored?.entered ?? []).map((entry) => [entry.stage, entry.state]));
  const boundaries = new Map((restored?.boundaries ?? []).map((entry) => [entry.stage, entry.boundary]));
  let pager: Stepper | null = null;

  const applyButtonTokens = createButtonTokenReplacer(() => [
    ["$checkButton", "$b{Check}"],
    ["$resetButton", "$b{Reset}"],
  ]);
  const { hide: hideHint, show: showHint } = createHintPresenter(ui.hintPanel, applyButtonTokens);

  const current = () => (position > 0 ? stages[position - 1]! : null);
  const needsSolve = (stage: Stage | null, index: number) => !!stage && stage.mode !== "none" && index > solved;
  const persist = () =>
    progress.save({
      position,
      solved,
      entered: [...entered].map(([stage, state]) => ({ stage, state })),
      boundaries: [...boundaries].map(([stage, boundary]) => ({ stage, boundary })),
    });

  function stageLabel(stage: Stage | undefined): string {
    if (!stage) return "";
    const line = stage.step.line + 1;
    const verb = stage.step.op === "JZ" || stage.step.op === "JNZ"
      ? "Branch from"
      : JUMPS.has(stage.step.op)
        ? "Jump from"
        : "Run";
    return `${needsSolve(stage, stages.indexOf(stage)) ? "🔧 " : ""}${verb} line ${line}`;
  }

  // Lines that have run in the current pass. A jump backwards, like the end of a
  // loop, starts a new pass over the lines it jumps back over. (Returning from a
  // subroutine goes backwards too, but it's not a loop.)
  function doneLines(upTo: number): Set<number> {
    const done = new Set<number>();
    for (let i = 0; i < upTo; i += 1) {
      const line = stages[i]!.step.line;
      const previous = stages[i - 1]?.step;
      if (previous && previous.jumped && ["JMP", "JZ", "JNZ"].includes(previous.op) && line <= previous.line) {
        for (const shown of [...done]) if (shown >= line && shown <= previous.line) done.delete(shown);
      }
      // A call starts the subroutine over, up to its RET, like a C function call.
      if (previous?.op === "CALL") {
        const ret = program.instructions.find((instruction) => instruction.line >= line && instruction.op === "RET");
        const end = ret?.line ?? lines.length - 1;
        for (const shown of [...done]) if (shown >= line && shown <= end) done.delete(shown);
      }
      done.add(line);
    }
    return done;
  }

  function renderCode(stage: Stage | null, solving: boolean) {
    const index = position - 1;
    const resolvedUpTo = solving ? position - 1 : position;
    const boundary = stage ? (solving ? stage.step.line : stage.afterBoundary) : (program.instructions[0]?.line ?? 0);
    const upcoming = !solving ? stages[position] : undefined;
    const selectable = solving && stage?.mode === "boundary"
      ? Array.from({ length: lines.length + 1 }, (_, i) => i)
      : [];
    renderCodePane(ui.codeEl, lines, boundary, {
      doneLines: doneLines(resolvedUpTo),
      nextRange: upcoming ? { start: upcoming.step.line, end: upcoming.step.line } : undefined,
      progress: solving,
      progressRange: solving && stage ? { start: stage.step.line, end: stage.step.line } : undefined,
      hideBoundary: selectable.length > 0,
      boundaryTargets: selectable.length > 0,
      selectableBoundaries: selectable,
      selectedBoundary: boundaries.get(index) ?? null,
    });
    ui.codeEl.querySelectorAll<HTMLElement>(".boundary.selectable").forEach((node) => {
      node.addEventListener("click", () => {
        boundaries.set(index, Number(node.dataset.boundary));
        ui.statusEl.textContent = "";
        render();
      });
    });
    if (c && ui.cEl) {
      const cLines = c.code.replace(/\n$/, "").split("\n");
      const active = stage ? c.lines[stage.step.line] : (upcoming ? c.lines[upcoming.step.line] : undefined);
      renderCodePane(ui.cEl, cLines, 0, { hideBoundary: true, doneLines: new Set() });
      if (active !== undefined) ui.cEl.querySelectorAll(".line")[active]?.classList.add("machine-c-active");
    }
  }

  function render() {
    const stage = current();
    const index = position - 1;
    const solving = needsSolve(stage, index);
    const text = stage ? stage.instructions : initialInstructions;
    setPartsContent(ui.instructionsEl, applyButtonTokens(text || null));
    renderCode(stage, solving);
    clearNode(ui.stateEl);
    ui.actions.classList.toggle("hidden", !solving);
    ui.undoBtn.classList.toggle("hidden", !solving || stage?.mode !== "state");
    if (solving && stage?.mode === "state") {
      const state = entered.get(index) ?? copyMachine(stage.before);
      ui.stateEl.appendChild(renderMachineState(state, view, (changed) => {
        entered.set(index, changed);
        ui.statusEl.textContent = "";
        persist();
        render();
      }));
    } else {
      const shown = stage ? (solving ? stage.before : stage.step.after) : initial;
      ui.stateEl.appendChild(renderMachineState(shown, view));
    }
    hideHint();
    pager?.update();
    persist();
  }

  function check() {
    const stage = current();
    const index = position - 1;
    if (!stage || !needsSolve(stage, index)) return;
    const ok = stage.mode === "boundary"
      ? boundaries.get(index) === stage.afterBoundary
      : sameMachineState(entered.get(index) ?? stage.before, stage.step.after, view);
    if (stage.mode === "boundary" && !boundaries.has(index)) {
      ui.statusEl.textContent = "Select where execution continues.";
      ui.statusEl.className = "machine-status err";
      flashStatus(ui.statusEl);
      return;
    }
    if (!ok) {
      ui.statusEl.textContent = "incorrect";
      ui.statusEl.className = "machine-status err";
      flashStatus(ui.statusEl);
      return;
    }
    solved = Math.max(solved, index);
    entered.delete(index);
    render();
    ui.statusEl.textContent = "correct";
    ui.statusEl.className = "machine-status ok";
    flashStatus(ui.statusEl);
    pager?.pulseNext();
  }

  ui.checkBtn.addEventListener("click", check);
  ui.undoBtn.addEventListener("click", () => {
    entered.delete(position - 1);
    render();
  });
  ui.hintBtn.addEventListener("click", () => {
    const stage = current();
    if (!stage) return;
    const index = position - 1;
    const state = entered.get(index) ?? stage.before;
    if (stage.mode === "state" && sameMachineState(state, stage.step.after, view)) {
      showHint("Looks good. Press $checkButton.");
      return;
    }
    showHint(stage.hint ?? (stage.mode === "boundary"
      ? "A jump continues at its label, if it jumps. Otherwise, execution continues with the next line."
      : "Work out what this one instruction changes. Everything else stays the same."));
  });
  ui.resetBtn.addEventListener("click", () => resetLevelAfterConfirmation(progress));

  const endLabel = nextLessonLabel({ next, fallback: "Next Page", override: nextLabel });
  pager = createStepper({
    root: ui.section,
    lines: stages.length,
    nextPage: next,
    getBoundary: () => position,
    setBoundary: (value) => {
      position = Math.max(0, Math.min(stages.length, Math.floor(value)));
      ui.statusEl.textContent = "";
    },
    isStepLocked: () => needsSolve(current(), position - 1),
    onAfterChange: () => render(),
    getNextLabel: (_boundary, _total, atEnd) => (atEnd ? endLabel : stageLabel(stages[position]) || "Next"),
    endLabel,
    backToIntro: () => {
      position = 0;
      render();
    },
  });
  render();
}

interface MachineChallengeConfig {
  instructions: string;
  startCode?: string;
  // The test cases: what's in the registers and memory when the program starts.
  cases: MachineStart[];
  // A working program, whose results the learner's have to match.
  solve: string;
  // What has to match. Defaults to just the lights.
  check?: { lights?: boolean; registers?: Register[]; memory?: number[] };
  view?: MachineView;
  hints?: (ctx: { error: string | null; final: MachineState | null; expected: MachineState; start: MachineState }) => string | null;
  next: string | null;
  nextLabel?: string;
}

interface MachineChallengeProgress {
  text: string;
  shownCase: number;
  pass: boolean;
}

/** An exercise where the learner writes a program for the toy machine. */
export function createMachineChallengeTemplate(config: MachineChallengeConfig): void {
  const { instructions, startCode = "", cases, solve, view = {}, next, nextLabel } = config;
  const check = config.check ?? { lights: true };
  if (queryRole("machine-root")) return;
  if (!cases.length) invalidTemplateConfig("A machine challenge needs at least one case.");
  const reference = assemble(solve);
  const expected = cases.map((start, index) => {
    const result = runMachine(reference, startState(start));
    if (result.error) invalidTemplateConfig(`solve fails case ${index}: ${result.error}`);
    return result.final;
  });

  const resolvedTitle = syncDocumentTitleFromNav();
  const main = ensurePanelizedMain(resolvedTitle);
  const section = document.createElement("section");
  section.dataset.role = "machine-root";
  section.className = "panel-shell";
  section.innerHTML = `
    <div class="controls-bar">
      <div class="controls-main panel panel-controls">
        <div class="controls-row controls-left">
          <button data-stepper="prev">Back ◀</button>
          <button data-stepper="next">Next ▶</button>
          <span class="controls-spacer"></span>
          <button data-role="machine-reset-level">Reset level</button>
        </div>
      </div>
      <div class="hint-inline hidden"></div>
      <p class="intro"></p>
    </div>
    <div class="panel code-panel-shell">
      <div class="panel-title code-title">Assembly</div>
      <textarea class="machine-editor" spellcheck="false" aria-label="Assembly code"></textarea>
      <div class="machine-actions">
        <button type="button" data-role="machine-new-input">New input</button>
        <button type="button" data-role="machine-hint">Hint</button>
        <button type="button" data-role="machine-check">Check</button>
      </div>
      <div class="machine-status muted"></div>
    </div>
    <div class="machine-results"></div>
  `;
  main.appendChild(section);
  bindBtnRefPulse(section);
  const $ = <T extends Element>(selector: string) => section.querySelector<T>(selector)!;
  const editor = $<HTMLTextAreaElement>(".machine-editor");
  const statusEl = $<HTMLElement>(".machine-status");
  const results = $<HTMLElement>(".machine-results");
  const hintPanel = $<HTMLElement>(".hint-inline");
  setPartsContent($<HTMLElement>(".intro"), instructions);
  const { hide: hideHint, show: showHint } = createHintPresenter(hintPanel, (parts) => parts);

  const progress = createLevelProgressController<MachineChallengeProgress>(
    (snapshot) => snapshot.text === startCode && snapshot.shownCase === 0 && !snapshot.pass,
  );
  const restored = progress.restore();
  let shownCase = Math.min(cases.length - 1, Math.max(0, restored?.shownCase ?? 0));
  let pass = restored?.pass ?? false;
  editor.value = restored?.text ?? startCode;
  editor.rows = Math.max(10, startCode.split("\n").length + 2);
  let pager: Stepper | null = null;
  const persist = () => progress.save({ text: editor.value, shownCase, pass });

  const matches = (final: MachineState, wanted: MachineState) =>
    (check.lights === false || final.memory[LIGHTS_ADDRESS] === wanted.memory[LIGHTS_ADDRESS]) &&
    (check.registers ?? []).every((register) => final.registers[register] === wanted.registers[register]) &&
    (check.memory ?? []).every((address) => final.memory[address] === wanted.memory[address]);

  function attempt(index: number): { error: string | null; final: MachineState | null } {
    try {
      const result = runMachine(assemble(editor.value), startState(cases[index]));
      return { error: result.error, final: result.final };
    } catch (error) {
      const line = error instanceof MachineError ? ` (line ${error.line + 1})` : "";
      return { error: `${error instanceof Error ? error.message : String(error)}${line}`, final: null };
    }
  }

  function describeStart(start: MachineStart): string {
    const parts = Object.entries(start.registers ?? {}).map(([register, value]) => `${register} = ${value}`);
    for (const [address, value] of Object.entries(start.memory ?? {})) parts.push(`address ${address} = ${value}`);
    return parts.length ? `Starting with ${parts.join(", ")}.` : "";
  }

  function renderResults() {
    clearNode(results);
    const start = startState(cases[shownCase]);
    const ran = attempt(shownCase);
    const column = (title: string, state: MachineState | null, note?: string) => {
      const box = document.createElement("div");
      box.className = "panel machine-panel";
      const heading = document.createElement("div");
      heading.className = "panel-title";
      heading.textContent = title;
      box.appendChild(heading);
      if (note) {
        const text = document.createElement("div");
        text.className = "machine-status";
        text.textContent = note;
        box.appendChild(text);
      }
      if (state) box.appendChild(renderMachineState(state, view));
      results.appendChild(box);
    };
    column("Your program's result", ran.final, ran.error ?? describeStart(cases[shownCase]!));
    column("Expected result", expected[shownCase]!);
    void start;
  }

  section.querySelector("[data-role=machine-check]")!.addEventListener("click", () => {
    hideHint();
    const failing = cases.findIndex((_, index) => {
      const ran = attempt(index);
      return ran.error !== null || !ran.final || !matches(ran.final, expected[index]!);
    });
    pass = failing < 0;
    if (!pass) shownCase = failing;
    statusEl.textContent = pass ? "correct" : "incorrect: here's an input it gets wrong";
    statusEl.className = `machine-status ${pass ? "ok" : "err"}`;
    flashStatus(statusEl);
    renderResults();
    persist();
    pager?.update();
    if (pass) pager?.pulseNext();
  });
  section.querySelector("[data-role=machine-new-input]")!.addEventListener("click", () => {
    shownCase = (shownCase + 1) % cases.length;
    renderResults();
    persist();
  });
  section.querySelector("[data-role=machine-hint]")!.addEventListener("click", () => {
    const ran = attempt(shownCase);
    const hint = config.hints?.({ ...ran, expected: expected[shownCase]!, start: startState(cases[shownCase]) });
    showHint(
      hint ??
        (ran.error
          ? ran.error
          : ran.final && matches(ran.final, expected[shownCase]!)
            ? "This input works. Press Check to try all of them."
            : "Compare your program's result with the expected result, and step through your program in your head."),
    );
  });
  section.querySelector("[data-role=machine-reset-level]")!.addEventListener("click", () => resetLevelAfterConfirmation(progress));
  editor.addEventListener("input", () => {
    pass = false;
    statusEl.textContent = "";
    renderResults();
    persist();
    pager?.update();
  });

  const endLabel = nextLessonLabel({ next, fallback: "Next Page", override: nextLabel });
  pager = createStepper({
    root: section,
    lines: 0,
    nextPage: next,
    getBoundary: () => 0,
    setBoundary: () => {},
    isStepLocked: () => !pass,
    getNextLabel: () => endLabel,
    endLabel,
  });
  renderResults();
}
