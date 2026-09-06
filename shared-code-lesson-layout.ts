import {
  ensurePanelizedMain,
  queryElement,
  queryRole,
  syncDocumentTitleFromNav,
} from "./shared-core.js";

interface CodeLessonElements {
  instructionsEl: HTMLElement | null;
  lockedLineNumbers: HTMLElement | null;
  lockedInputLine: HTMLElement | null;
  editor: HTMLTextAreaElement | null;
  lineNumbers: HTMLElement | null;
  stage: HTMLElement | null;
  status: HTMLElement | null;
  diagnosticEl: HTMLElement | null;
  hintPanel: HTMLElement | null;
  hintBtn: HTMLButtonElement | null;
  checkBtn: HTMLButtonElement | null;
  levelResetBtn: HTMLButtonElement | null;
  rerollBtn: HTMLButtonElement | null;
  showFailBtn: HTMLButtonElement | null;
  prevBtn: HTMLButtonElement | null;
  nextBtn: HTMLButtonElement | null;
  codeRoot: HTMLElement | null;
}

interface CodeLessonLayoutOptions {
  textareaMinLines: number;
  lockedInput: boolean;
}

function collectCodeLessonElements(
  root: ParentNode = document,
): CodeLessonElements {
  const role = <T extends Element>(name: string) => queryRole<T>(name, root);
  const rootElement = root instanceof HTMLElement ? root : null;
  return {
    instructionsEl: role<HTMLElement>("code-instructions"),
    lockedLineNumbers: role<HTMLElement>("code-locked-line-numbers"),
    lockedInputLine: role<HTMLElement>("code-locked-input-line"),
    editor: role<HTMLTextAreaElement>("code-editor"),
    lineNumbers: role<HTMLElement>("code-line-numbers"),
    stage: role<HTMLElement>("code-stage"),
    status: role<HTMLElement>("code-status"),
    diagnosticEl: role<HTMLElement>("code-diagnostic"),
    hintPanel: role<HTMLElement>("code-hint"),
    hintBtn: role<HTMLButtonElement>("code-hint-btn"),
    checkBtn: role<HTMLButtonElement>("code-check"),
    levelResetBtn: role<HTMLButtonElement>("code-reset-level"),
    rerollBtn: role<HTMLButtonElement>("code-reroll"),
    showFailBtn: role<HTMLButtonElement>("code-show-failing-case"),
    prevBtn: queryElement<HTMLButtonElement>('button[data-stepper="prev"]', root),
    nextBtn: queryElement<HTMLButtonElement>('button[data-stepper="next"]', root),
    codeRoot:
      rootElement?.dataset.role === "code-root"
        ? rootElement
        : role<HTMLElement>("code-root"),
  };
}

function lessonButton({
  text,
  role,
  stepper,
  className,
}: {
  text: string;
  role?: string;
  stepper?: "prev" | "next";
  className?: string;
}): HTMLButtonElement {
  const button = document.createElement("button");
  button.type = "button";
  button.textContent = text;
  if (role) button.dataset.role = role;
  if (stepper) button.dataset.stepper = stepper;
  if (className) button.className = className;
  return button;
}

function ensureCodeLessonLayout({
  textareaMinLines,
  lockedInput,
}: CodeLessonLayoutOptions): CodeLessonElements {
  const resolvedTitle = syncDocumentTitleFromNav();
  if (queryRole<HTMLElement>("code-editor")) {
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
  if (lockedInput) {
    const lockedRow = document.createElement("div");
    lockedRow.className = "codepane-row code-locked-row";
    const lockedNumbers = document.createElement("div");
    lockedNumbers.dataset.role = "code-locked-line-numbers";
    lockedNumbers.className = "code-gutter";
    lockedNumbers.setAttribute("aria-hidden", "true");
    const lockedLine = document.createElement("div");
    lockedLine.dataset.role = "code-locked-input-line";
    lockedLine.className = "code-locked-line";
    lockedRow.append(lockedNumbers, lockedLine);
    codePane.appendChild(lockedRow);
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
  editor.spellcheck = false;
  editor.rows = Math.max(1, Math.floor(textareaMinLines));
  editorWrap.appendChild(editor);
  codeRow.append(lineNumbers, editorWrap);
  codePane.appendChild(codeRow);
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
  controlsRow.append(
    lessonButton({ text: "Reset level", role: "code-reset-level" }),
    lessonButton({ text: "Hint", role: "code-hint-btn", className: "hint-link" }),
    lessonButton({ text: "Check", role: "code-check" }),
  );
  if (lockedInput) {
    controlsRow.appendChild(
      lessonButton({
        text: "Show failing case",
        role: "code-show-failing-case",
        className: "hidden",
      }),
    );
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

  return collectCodeLessonElements(section);
}

export { ensureCodeLessonLayout };
export type { CodeLessonElements, CodeLessonLayoutOptions };
