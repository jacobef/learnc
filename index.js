{
  const { ensureBaseLayout } = window.MB;

  const { main } = ensureBaseLayout();
  if (main) {
    document.title = "C Boxes - Home";

    const heading = document.createElement("h1");
    heading.textContent = "C Boxes";
    main.appendChild(heading);

    const intro = document.createElement("div");
    intro.className = "intro";

    const mainCopy = document.createElement("p");
    mainCopy.appendChild(
      document.createTextNode(
        "This is a work-in-progress tutorial that teaches the C programming language. I'll try to update it with a new program every other day or so.",
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
      document.createTextNode("Remember to always read the instructions!"),
    );
    mainCopy.appendChild(document.createElement("br"));
    mainCopy.appendChild(
      document.createTextNode(
        "Email feedback, bugs, etc to jacobef2@gmail.com.",
      ),
    );
    mainCopy.appendChild(document.createElement("br"));

    const mobileNote = document.createElement("p");
    mobileNote.className = "mobile-note";
    mobileNote.appendChild(
      document.createTextNode(
        "It's a bit rough on mobile at the moment, sorry!",
      ),
    );
    mobileNote.appendChild(document.createElement("br"));

    const updated = document.createElement("i");
    updated.textContent = "Site last updated January 5, 2026";

    intro.appendChild(mainCopy);
    intro.appendChild(mobileNote);
    intro.appendChild(updated);

    main.appendChild(intro);

    const startWrap = document.createElement("div");
    const startLink = document.createElement("a");
    startLink.href = "program1.html";
    const startButton = document.createElement("button");
    startButton.className = "start-button";
    startButton.textContent = "Start here!";
    startLink.appendChild(startButton);
    startWrap.appendChild(startLink);
    main.appendChild(startWrap);
  }
}
