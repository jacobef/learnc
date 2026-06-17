import { ensureBaseLayout } from "./shared-core.js";
import {
  clearAllLevelProgress,
  clearSandboxProgress,
  hasSandboxProgress,
  savedLevelCount,
} from "./shared-progress.js";

{
  const { main } = ensureBaseLayout();
  main.replaceChildren();
  document.title = "C Boxes - Home";

  const appendTextLines = (parent: HTMLElement, lines: string[]): void => {
    lines.forEach((line, index) => {
      if (index > 0) parent.appendChild(document.createElement("br"));
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
  updated.textContent = "Site last updated June 17, 2026";

  intro.appendChild(mainCopy);
  intro.appendChild(mobileNote);
  intro.appendChild(updated);

  main.appendChild(intro);

  const startWrap = document.createElement("div");
  startWrap.className = "home-actions";
  const startLink = document.createElement("a");
  const updateStartLink = (): void => {
    const sidebarState = document.body.classList.contains("sidebar-collapsed") ? "0" : "1";
    const startUrl = new URL("1-assignment-i.html", window.location.href);
    startUrl.searchParams.set("sidebar", sidebarState);
    startLink.href = startUrl.toString();
  };
  updateStartLink();
  const observer = new MutationObserver(updateStartLink);
  observer.observe(document.body, {
    attributes: true,
    attributeFilter: ["class"],
  });
  window.addEventListener(
    "beforeunload",
    () => {
      observer.disconnect();
    },
    { once: true },
  );
  const startButton = document.createElement("button");
  startButton.className = "start-button";
  startButton.textContent = "Start here!";
  startLink.appendChild(startButton);
  startWrap.appendChild(startLink);
  const resetProgressBtn = document.createElement("button");
  const updateResetProgressButton = (): void => {
    const count = savedLevelCount() + (hasSandboxProgress() ? 1 : 0);
    resetProgressBtn.textContent =
      count > 0 ? `Reset all progress (${count})` : "Reset all progress";
    resetProgressBtn.disabled = count === 0;
  };
  resetProgressBtn.textContent = "Reset all progress";
  resetProgressBtn.addEventListener("click", () => {
    if (savedLevelCount() <= 0 && !hasSandboxProgress()) return;
    const confirmed = window.confirm(
      "Reset all saved progress? This clears every level's saved state and the sandbox.",
    );
    if (!confirmed) return;
    clearAllLevelProgress();
    clearSandboxProgress();
    updateResetProgressButton();
  });
  updateResetProgressButton();
  startWrap.appendChild(resetProgressBtn);
  main.appendChild(startWrap);
}
