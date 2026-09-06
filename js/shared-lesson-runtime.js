import { applyTextTokenReplacements, flashStatus, getNavLabelForHref, renderParts, } from "./shared-core.js";
import { clearLevelProgress, currentLevelId, readLevelProgress, writeLevelProgress, } from "./shared-progress.js";
function createLevelProgressController(isDefault, levelId = currentLevelId()) {
    return {
        restore: () => readLevelProgress(levelId),
        save: (state) => {
            if (isDefault(state))
                clearLevelProgress(levelId);
            else
                writeLevelProgress(state, levelId);
        },
        clear: () => clearLevelProgress(levelId),
    };
}
function createButtonTokenReplacer(replacements) {
    return (parts) => applyTextTokenReplacements(parts, replacements());
}
function createHintPresenter(panel, transform = (parts) => parts) {
    const hide = () => {
        if (!panel)
            return;
        panel.classList.add("hidden");
        panel.textContent = "";
    };
    const show = (parts) => {
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
function nextLessonLabel({ next, fallback, override, }) {
    if (override)
        return override;
    const label = getNavLabelForHref(next);
    return label ? `Next: ${label}` : fallback;
}
function resetLevelAfterConfirmation(progress, reset = () => window.location.reload()) {
    if (!window.confirm("Reset your saved progress for this level and start over?")) {
        return;
    }
    progress.clear();
    reset();
}
function invalidTemplateConfig(message) {
    window.alert(message);
    throw new Error(message);
}
export { createButtonTokenReplacer, createHintPresenter, createLevelProgressController, invalidTemplateConfig, nextLessonLabel, resetLevelAfterConfirmation, };
