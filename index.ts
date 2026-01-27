import { ensureBaseLayout } from "./shared-core.js";

{
  const { main } = ensureBaseLayout();
  document.title = "C Boxes - Home";

  const heading = document.createElement("h1");
  heading.textContent = "C Boxes";
  main.appendChild(heading);

  const intro = document.createElement("div");
  intro.className = "intro";

  const mainCopy = document.createElement("p");
  mainCopy.appendChild(
    document.createTextNode(
      "This is an unfinished, work-in-progress tutorial that teaches the C programming language.",
    ),
  );
  mainCopy.appendChild(document.createElement("br"));
  mainCopy.appendChild(
    document.createTextNode(
      'Treat it like a puzzle game; you might not understand certain elements the first time you see them. For example, an "address" first appears on the 1st page, but is not relevant until the 7th page.',
    ),
  );
  mainCopy.appendChild(document.createElement("br"));
  mainCopy.appendChild(
    document.createTextNode("Always read the instructions!"),
  );
  mainCopy.appendChild(document.createElement("br"));
  mainCopy.appendChild(
    document.createTextNode("Email feedback, bugs, etc to jacobef2@gmail.com."),
  );
  mainCopy.appendChild(document.createElement("br"));

  const mobileNote = document.createElement("p");
  mobileNote.className = "mobile-note";
  mobileNote.appendChild(
    document.createTextNode("It's a bit rough on mobile at the moment, sorry!"),
  );
  mobileNote.appendChild(document.createElement("br"));

  const updated = document.createElement("i");
  updated.textContent = "Site last updated January 26, 2026";

  intro.appendChild(mainCopy);
  intro.appendChild(mobileNote);
  intro.appendChild(updated);

  main.appendChild(intro);

  const startWrap = document.createElement("div");
  const startLink = document.createElement("a");
  const updateStartLink = (): void => {
    const sidebarState = document.body.classList.contains("sidebar-collapsed")
      ? "0"
      : "1";
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
  const startButton = document.createElement("button");
  startButton.className = "start-button";
  startButton.textContent = "Start here!";
  startLink.appendChild(startButton);
  startWrap.appendChild(startLink);
  main.appendChild(startWrap);
}
