import {
  applyTextTokenReplacements,
  flashStatus,
  getNavLabelForHref,
  renderParts,
} from "./shared-core.js";
import type { Parts } from "./shared-core.js";
import {
  clearLevelProgress,
  currentLevelId,
  readLevelProgress,
  writeLevelProgress,
} from "./shared-progress.js";

type ButtonReplacement = readonly [token: string, replacement: string];

interface LevelProgressController<T> {
  restore: () => T | null;
  save: (state: T) => void;
  clear: () => void;
}

interface HintPresenter {
  hide: () => void;
  show: (parts: Parts | null | undefined) => boolean;
}

function createLevelProgressController<T>(
  isDefault: (state: T) => boolean,
  levelId: string = currentLevelId(),
): LevelProgressController<T> {
  return {
    restore: () => readLevelProgress<T>(levelId),
    save: (state) => {
      if (isDefault(state)) clearLevelProgress(levelId);
      else writeLevelProgress(state, levelId);
    },
    clear: () => clearLevelProgress(levelId),
  };
}

function createButtonTokenReplacer(
  replacements: () => readonly ButtonReplacement[],
): (parts: Parts | null) => Parts | null {
  return (parts) =>
    applyTextTokenReplacements(parts, replacements()) as Parts | null;
}

function createHintPresenter(
  panel: HTMLElement | null,
  transform: (parts: Parts | null) => Parts | null = (parts) => parts,
): HintPresenter {
  const hide = () => {
    if (!panel) return;
    panel.classList.add("hidden");
    panel.textContent = "";
  };
  const show = (parts: Parts | null | undefined) => {
    const rendered = transform(parts ?? null);
    if (!panel || !rendered || (Array.isArray(rendered) && rendered.length === 0)) {
      return false;
    }
    renderParts(panel, rendered);
    panel.classList.remove("hidden");
    flashStatus(panel);
    return true;
  };
  return { hide, show };
}

function nextLessonLabel({
  next,
  fallback,
  override,
}: {
  next: string | null;
  fallback: string;
  override?: string;
}): string {
  if (override) return override;
  const label = getNavLabelForHref(next);
  return label ? `Next: ${label}` : fallback;
}

function resetLevelAfterConfirmation(
  progress: Pick<LevelProgressController<unknown>, "clear">,
  reset: () => void = () => window.location.reload(),
): void {
  if (!window.confirm("Reset your saved progress for this level and start over?")) {
    return;
  }
  progress.clear();
  reset();
}

function invalidTemplateConfig(message: string): never {
  window.alert(message);
  throw new Error(message);
}

export {
  createButtonTokenReplacer,
  createHintPresenter,
  createLevelProgressController,
  invalidTemplateConfig,
  nextLessonLabel,
  resetLevelAfterConfirmation,
};
export type { HintPresenter, LevelProgressController };
