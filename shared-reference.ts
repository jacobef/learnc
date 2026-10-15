import { DEFAULT_NAV_ITEMS } from "./nav-items.js";
import { REFERENCE_SECTIONS } from "./shared-reference-data.js";
import type { ReferenceEntry, ReferenceSection } from "./shared-reference-data.js";

type RenderStyledText = (target: HTMLElement, text: string) => void;

// Lessons are matched by name without their number, so entries follow a lesson
// when lessons are renumbered.
const lessonSlug = (href: string) => href.replace(/^\d+-/, "");

function navIndex(href: string): number {
  return DEFAULT_NAV_ITEMS.findIndex((item) => lessonSlug(item.href) === lessonSlug(href));
}

// Only content from lessons up to and including the current page is shown.
function sectionsUpTo(currentIndex: number): ReferenceSection[] {
  const reached = (entry: ReferenceEntry) => {
    const index = navIndex(entry.lesson);
    return index >= 0 && index <= currentIndex;
  };
  return REFERENCE_SECTIONS.map((section) => ({
    ...section,
    notes: section.notes?.filter(reached),
    groups: section.groups
      .map((group) => ({ ...group, entries: group.entries.filter(reached) }))
      .filter((group) => group.entries.length > 0),
  })).filter((section) => section.groups.length > 0);
}

function entryNode(entry: ReferenceEntry, render: RenderStyledText): HTMLElement {
  const row = document.createElement("div");
  row.className = "reference-entry";
  const code = document.createElement("code");
  code.className = "reference-code";
  code.textContent = entry.code;
  const text = document.createElement("div");
  text.className = "reference-text";
  render(text, entry.text);
  const lesson = document.createElement("a");
  lesson.className = "reference-lesson";
  const item = DEFAULT_NAV_ITEMS[navIndex(entry.lesson)];
  lesson.href = item?.href ?? entry.lesson;
  const label = item?.label ?? "";
  lesson.textContent = label.replace(/\..*$/, "");
  lesson.title = `Introduced in ${label}`;
  row.append(code, text, lesson);
  return row;
}

function buildDialog(sections: ReferenceSection[], render: RenderStyledText): HTMLDialogElement {
  const dialog = document.createElement("dialog");
  dialog.className = "reference-dialog";
  dialog.setAttribute("aria-labelledby", "reference-title");

  const header = document.createElement("div");
  header.className = "reference-header";
  const title = document.createElement("h2");
  title.id = "reference-title";
  title.textContent = "Quick reference";
  const close = document.createElement("button");
  close.type = "button";
  close.className = "reference-close";
  close.setAttribute("aria-label", "Close");
  close.textContent = "×";
  close.addEventListener("click", () => dialog.close());
  header.append(title, close);

  const body = document.createElement("div");
  body.className = "reference-body";
  const intro = document.createElement("p");
  intro.className = "reference-intro";
  intro.textContent =
    "This covers everything up to the current lesson, and grows as you go. The number on the right of each row is the lesson that introduced it.";
  body.appendChild(intro);
  for (const section of sections) {
    const heading = document.createElement("h3");
    heading.textContent = section.title;
    body.appendChild(heading);
    for (const note of section.notes ?? []) {
      const p = document.createElement("p");
      p.className = "reference-note";
      render(p, note.text);
      body.appendChild(p);
    }
    for (const group of section.groups) {
      const groupEl = document.createElement("div");
      groupEl.className = "reference-group";
      if (group.label) {
        const label = document.createElement("div");
        label.className = "reference-group-label";
        label.textContent = group.label;
        groupEl.appendChild(label);
      }
      group.entries.forEach((entry) => groupEl.appendChild(entryNode(entry, render)));
      body.appendChild(groupEl);
    }
  }

  dialog.append(header, body);
  dialog.addEventListener("click", (event) => {
    if (event.target !== dialog) return;
    const bounds = dialog.getBoundingClientRect();
    if (event.clientX < bounds.left || event.clientX > bounds.right ||
        event.clientY < bounds.top || event.clientY > bounds.bottom) {
      dialog.close();
    }
  });
  return dialog;
}

function mountQuickReference(
  container: HTMLElement,
  currentHref: string,
  render: RenderStyledText,
): void {
  if (container.querySelector(".reference-toggle")) return;
  const sections = sectionsUpTo(navIndex(currentHref));
  if (!sections.length) return;
  const dialog = buildDialog(sections, render);
  const button = document.createElement("button");
  button.type = "button";
  button.className = "reference-toggle";
  button.textContent = "Reference";
  button.setAttribute("aria-haspopup", "dialog");
  button.addEventListener("click", () => dialog.showModal());
  container.appendChild(button);
  document.body.appendChild(dialog);
}

export { mountQuickReference };
