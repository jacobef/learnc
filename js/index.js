import { ensureBaseLayout } from "./shared-core.js";
import { setReplaySharing } from "./shared-replay.js";
import { clearAllLevelProgress, clearSandboxProgress, hasSandboxProgress, savedLevelCount, } from "./shared-progress.js";
{
    const { main } = ensureBaseLayout();
    main.replaceChildren();
    document.title = "C Boxes - Home";
    const appendTextLines = (parent, lines) => {
        lines.forEach((line, index) => {
            if (index > 0)
                parent.appendChild(document.createElement("br"));
            parent.appendChild(document.createTextNode(line));
        });
    };
    const heading = document.createElement("h1");
    heading.textContent = "C Boxes";
    main.appendChild(heading);
    const intro = document.createElement("div");
    intro.className = "intro home-intro";
    const mainCopy = document.createElement("p");
    appendTextLines(mainCopy, [
        "This is an unfinished, work-in-progress tutorial that teaches the C programming language.",
        'Treat it like a puzzle game; you might not understand certain elements the first time you see them. For example, an "address" first appears on the 1st page, but is not relevant until the 7th page.',
        "Always read the instructions!",
        "Email feedback, bugs, etc to jacobef2@gmail.com.",
        "",
    ]);
    const mobileNote = document.createElement("p");
    mobileNote.className = "mobile-note";
    appendTextLines(mobileNote, ["It's a bit rough on mobile at the moment, sorry!", ""]);
    const updated = document.createElement("i");
    updated.textContent = "Site last updated September 13, 2026";
    intro.appendChild(mainCopy);
    intro.appendChild(mobileNote);
    intro.appendChild(updated);
    main.appendChild(intro);
    const startWrap = document.createElement("div");
    startWrap.className = "home-actions";
    const replayQuestion = document.createElement("dialog");
    replayQuestion.className = "replay-question";
    replayQuestion.setAttribute("aria-labelledby", "replay-question-text");
    replayQuestion.addEventListener("click", (event) => {
        if (event.target !== replayQuestion)
            return;
        const bounds = replayQuestion.getBoundingClientRect();
        if (event.clientX < bounds.left || event.clientX > bounds.right ||
            event.clientY < bounds.top || event.clientY > bounds.bottom) {
            replayQuestion.close();
        }
    });
    const question = document.createElement("p");
    question.id = "replay-question-text";
    question.textContent = "Share your level replays to help improve this tutorial?";
    replayQuestion.appendChild(question);
    const choices = document.createElement("div");
    choices.className = "home-actions";
    for (const [label, enabled] of [["Yes", true], ["No", false]]) {
        const choice = document.createElement("button");
        choice.textContent = label;
        choice.addEventListener("click", () => {
            try {
                setReplaySharing(enabled);
            }
            catch { /* Unavailable storage disables recording. */ }
            replayQuestion.close();
            const startUrl = new URL("1-assignment-i.html", window.location.href);
            startUrl.searchParams.set("sidebar", document.body.classList.contains("sidebar-collapsed") ? "0" : "1");
            window.location.assign(startUrl.toString());
        });
        choices.appendChild(choice);
    }
    replayQuestion.appendChild(choices);
    main.appendChild(replayQuestion);
    const startButton = document.createElement("button");
    startButton.className = "start-button";
    startButton.textContent = "Start here!";
    startButton.addEventListener("click", () => replayQuestion.showModal());
    startWrap.appendChild(startButton);
    const resetProgressBtn = document.createElement("button");
    const updateResetProgressButton = () => {
        const count = savedLevelCount() + (hasSandboxProgress() ? 1 : 0);
        resetProgressBtn.textContent =
            count > 0 ? `Reset all progress (${count})` : "Reset all progress";
        resetProgressBtn.disabled = count === 0;
    };
    resetProgressBtn.textContent = "Reset all progress";
    resetProgressBtn.addEventListener("click", () => {
        if (savedLevelCount() <= 0 && !hasSandboxProgress())
            return;
        const confirmed = window.confirm("Reset all saved progress? This clears every level's saved state and the sandbox.");
        if (!confirmed)
            return;
        clearAllLevelProgress();
        clearSandboxProgress();
        updateResetProgressButton();
    });
    updateResetProgressButton();
    startWrap.appendChild(resetProgressBtn);
    main.appendChild(startWrap);
}
