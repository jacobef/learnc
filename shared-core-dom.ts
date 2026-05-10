import type { BoxState, BoxValue } from "./shared-core-utils.js";
import { DEFAULT_NAV_ITEMS as NAV_ITEMS } from "./nav-items.js";
import {
  doubleDisplayIsExact,
  formatDoubleDefault,
  formatDoubleExact,
  formatDoubleStorage,
  getPointerDepth,
  normalizeZeroDisplay,
  parseDoubleValueWithSign,
  parseType,
  randAddr,
} from "./shared-core-utils.js";

export interface NavItem {
  href: string;
  label: string;
}
export interface RenderCodePaneOptions {
  progress?: boolean;
  progressIndex?: number;
  progressRange?: [number, number] | { start: number; end: number };
  doneBoundary?: number;
  hideBoundary?: boolean;
  selectableBoundaries?: number[];
  selectedBoundary?: number | null;
  suppressProgressMid?: boolean;
  boundaryTargets?: boolean;
  strikeRange?: [number, number] | { start: number; end: number };
  strikeRanges?: Array<[number, number] | { start: number; end: number }>;
  strikeFragments?: Array<{ line: number; start: number; end: number }>;
}
export interface TokenPart {
  kind: "tok";
  role: string;
  text: string;
}
export type Part = string;
export type Parts = string | string[];
export interface StepperOptions {
  root?: ParentNode | null;
  prevButtons?: HTMLButtonElement[] | null;
  nextButtons?: HTMLButtonElement[] | null;
  lines?: number | string[];
  nextPage?: string | null;
  getBoundary?: () => number;
  setBoundary?: (value: number) => void;
  onBeforeChange?: (current: number) => void;
  onAfterChange?: (current: number) => void;
  isStepLocked?: (current: number, atEnd: boolean) => boolean;
  getStepBadge?: (step: number) => string;
  getNextLabel?: (current: number, total: number, atEnd: boolean) => string;
  getNextBoundary?: (current: number, total: number) => number;
  getPrevBoundary?: (current: number, total: number) => number;
  isAtEnd?: (current: number, total: number) => boolean;
  endLabel?: string;
  allowSameBoundary?: boolean;
}
export interface Stepper {
  update: () => void;
  goTo: (target: number) => void;
  boundary: () => number;
  clearPulse: () => void;
  pulseNext: () => void;
}

type QueryRoot = ParentNode | null | undefined;

function queryElement<T extends Element>(
  selector: string,
  root: QueryRoot = document,
): T | null {
  return (root?.querySelector?.(selector) ?? null) as T | null;
}

function queryRole<T extends Element>(
  role: string,
  root: QueryRoot = document,
): T | null {
  return queryElement<T>(`[data-role="${role}"]`, root);
}

function clearNode(node: Element | null | undefined): void {
  node?.replaceChildren();
}

function el(html: string): HTMLElement {
  const t = document.createElement("template");
  t.innerHTML = html.trim();
  return t.content.firstElementChild as HTMLElement;
}

function txt(n?: Node | null): string {
  return (n?.textContent || "").trim();
}

function onDomReady(fn: () => void, { once = true }: { once?: boolean } = {}) {
  if (document.readyState === "loading") {
    document.addEventListener(
      "DOMContentLoaded",
      () => {
        fn();
      },
      once ? { once: true } : undefined,
    );
    return;
  }
  fn();
}

function disableAutoText(el?: Element | null) {
  if (!el || el.nodeType !== 1) return;
  el.setAttribute("autocapitalize", "off");
  el.setAttribute("autocorrect", "off");
  el.setAttribute("autocomplete", "off");
  el.setAttribute("spellcheck", "false");
}

function applyAutoTextDefaults(root: ParentNode = document) {
  root
    .querySelectorAll(
      'input[type="text"], input:not([type]), textarea, [contenteditable="true"]',
    )
    .forEach((el) => disableAutoText(el));
}

const MOBILE_MEDIA_QUERY = "(max-width: 900px)";

function isMobileViewport(): boolean {
  return window.matchMedia && window.matchMedia(MOBILE_MEDIA_QUERY).matches;
}

const DEFAULT_NAV_ITEMS: NavItem[] = NAV_ITEMS;

function resolveNavItems(items?: NavItem[]): NavItem[] {
  return items?.length ? items : DEFAULT_NAV_ITEMS;
}

function normalizeNavHref(href = ""): string {
  const clean = String(href || "")
    .split("#")[0]
    .split("?")[0];
  const parts = clean.split("/").filter(Boolean);
  return parts[parts.length - 1] || "index.html";
}

function getNavLabelForHref(href: string | null | undefined): string | null {
  if (!href) return null;
  const target = normalizeNavHref(href);
  const match = DEFAULT_NAV_ITEMS.find(
    (item) => normalizeNavHref(item?.href || "") === target,
  );
  const label = match?.label || "";
  if (!label) return null;
  return label.replace(/^\d+\.\s*/, "");
}

function currentNavHref(): string {
  const pathname = window.location?.pathname || "";
  const cleaned = normalizeNavHref(pathname);
  return cleaned || "index.html";
}

function resolveActiveNavItem(
  items: NavItem[] = DEFAULT_NAV_ITEMS,
  activeHref?: string,
): NavItem | undefined {
  const list = resolveNavItems(items);
  const current = normalizeNavHref(activeHref || currentNavHref());
  return list.find((item) => normalizeNavHref(item?.href || "") === current);
}

function buildNav(
  items: NavItem[] = DEFAULT_NAV_ITEMS,
  { activeHref }: { activeHref?: string } = {},
): HTMLElement {
  const list = resolveNavItems(items);
  const current = normalizeNavHref(activeHref || currentNavHref());
  const nav = document.createElement("nav");
  nav.className = "tabs";
  list.forEach((item) => {
    const link = document.createElement("a");
    link.href = item.href;
    link.textContent = item.label;
    if (normalizeNavHref(item.href) === current) {
      link.classList.add("active");
      link.setAttribute("aria-current", "page");
    }
    nav.appendChild(link);
  });
  return nav;
}

function findExistingLayoutNodes(wrap: HTMLElement | null): {
  nav: HTMLElement | null;
  main: HTMLElement | null;
} {
  const nav = (wrap?.querySelector("nav.tabs") ||
    document.querySelector("nav.tabs")) as HTMLElement | null;
  const main = (wrap?.querySelector(".main") ||
    document.querySelector(".main")) as HTMLElement | null;
  return { nav, main };
}

function ensureWrapConnected(wrap: HTMLElement, nav: HTMLElement, main: HTMLElement) {
  if (wrap.isConnected) return;
  const mount = document.body;
  const firstScript = mount.querySelector("script");
  const anchor =
    main.parentElement === mount ? main : nav.parentElement === mount ? nav : null;
  if (anchor) mount.insertBefore(wrap, anchor);
  else if (firstScript) mount.insertBefore(wrap, firstScript);
  else mount.appendChild(wrap);
}

function updateSidebarToggleLabel(btn: HTMLButtonElement) {
  const hidden = document.body.classList.contains("sidebar-collapsed");
  const label = hidden ? "Show sidebar" : "Hide sidebar";
  btn.classList.toggle("is-expanded", !hidden);
  btn.setAttribute("aria-label", label);
  btn.setAttribute("aria-expanded", hidden ? "false" : "true");
  const sr = btn.querySelector(".sr-only");
  if (sr) sr.textContent = label;
}

function updateSidebarQueryParam() {
  const hidden = document.body.classList.contains("sidebar-collapsed");
  const params = new URLSearchParams(window.location.search);
  params.set("sidebar", hidden ? "0" : "1");
  const query = params.toString();
  const next = `${window.location.pathname}?${query}${window.location.hash}`;
  window.history.replaceState(null, "", next);
}

function ensureSidebarControls(wrap: HTMLElement, nav: HTMLElement) {
  if (!nav.id) nav.id = "sidebar";
  let sidebarWrap = wrap.querySelector(".sidebar-wrap") as HTMLElement | null;
  if (!sidebarWrap) {
    sidebarWrap = document.createElement("div");
    sidebarWrap.className = "sidebar-wrap";
    wrap.insertBefore(sidebarWrap, wrap.firstChild);
  }
  if (nav.parentElement !== sidebarWrap) {
    sidebarWrap.appendChild(nav);
  }

  let btn = sidebarWrap.querySelector(
    ".sidebar-toggle",
  ) as HTMLButtonElement | null;
  if (!btn) {
    btn = el(
      '<button type="button" class="sidebar-toggle"><span class="hamburger" aria-hidden="true"><span></span><span></span><span></span></span><span class="sr-only">Toggle sidebar</span></button>',
    ) as HTMLButtonElement;
    sidebarWrap.insertBefore(btn, sidebarWrap.firstChild);
  } else if (btn.parentElement !== sidebarWrap) {
    sidebarWrap.insertBefore(btn, sidebarWrap.firstChild);
  }
  btn.setAttribute("aria-controls", nav.id);
  if (btn.dataset.bound !== "1") {
    btn.dataset.bound = "1";
    btn.addEventListener("click", () => {
      document.body.classList.toggle("sidebar-collapsed");
      updateSidebarToggleLabel(btn);
      updateSidebarQueryParam();
    });
  }
  updateSidebarToggleLabel(btn);
}

function ensureBaseLayout({
  navItems,
  activeHref,
}: { navItems?: NavItem[]; activeHref?: string } = {}) {
  let wrap = document.querySelector(".wrap") as HTMLElement | null;
  const existing = findExistingLayoutNodes(wrap);
  let nav = existing.nav;
  let main = existing.main;
  if (!wrap) {
    wrap = document.createElement("div");
    wrap.className = "wrap";
  }
  if (nav && nav.closest(".wrap") !== wrap) {
    nav.parentElement?.removeChild(nav);
  }
  if (main && main.closest(".wrap") !== wrap) {
    main.parentElement?.removeChild(main);
  }
  if (!nav) {
    nav = buildNav(navItems, { activeHref });
  }
  if (!main) {
    main = document.createElement("div");
    main.className = "main";
  }
  ensureWrapConnected(wrap, nav, main);
  if (!wrap.contains(nav)) {
    wrap.appendChild(nav);
  }
  if (main.parentElement !== wrap) {
    wrap.appendChild(main);
  }
  applySidebarStateFromUrl();
  ensureSidebarControls(wrap, nav);
  document.body.classList.add("panel-layout");
  requestAnimationFrame(() => {
    centerActiveNavItem(nav);
  });
  return {
    wrap: wrap as HTMLElement,
    nav: nav as HTMLElement,
    main: main as HTMLElement,
  };
}

function syncDocumentTitleFromNav(prefix = "C Boxes"): string {
  const activeItem = resolveActiveNavItem();
  const resolvedTitle = String(activeItem?.label || "").trim();
  document.title = resolvedTitle ? `${prefix} - ${resolvedTitle}` : prefix;
  return resolvedTitle;
}

function ensurePanelizedMain(title = ""): HTMLElement {
  const { main } = ensureBaseLayout();
  main.classList.add("main-panelized");
  if (title && !main.querySelector(".page-title")) {
    const heading = document.createElement("h1");
    heading.className = "page-title";
    heading.textContent = title;
    main.appendChild(heading);
  }
  return main;
}

function centerActiveNavItem(nav: HTMLElement) {
  const active = nav.querySelector("a.active") as HTMLElement | null;
  if (!active) return;
  const activeCenter = active.offsetTop + active.offsetHeight / 2;
  const target = activeCenter - nav.clientHeight / 2;
  const maxScroll = Math.max(0, nav.scrollHeight - nav.clientHeight);
  nav.scrollTop = Math.max(0, Math.min(maxScroll, target));
}

type LineRange = [number, number] | { start: number; end: number };

function clampLineIndex(index: number, maxIndex: number): number {
  return Math.max(0, Math.min(maxIndex, index));
}

function normalizeLineRange(
  range: LineRange,
  maxIndex: number,
): [number, number] {
  const startRaw = Number(Array.isArray(range) ? range[0] : range.start);
  const endRaw = Number(Array.isArray(range) ? range[1] : range.end);
  const start = clampLineIndex(Math.min(startRaw, endRaw), maxIndex);
  const end = clampLineIndex(Math.max(startRaw, endRaw), maxIndex);
  return [start, end];
}

function renderCodePane(
  root: Element,
  lines: string[],
  boundary: number,
  opts: RenderCodePaneOptions = {},
) {
  clearNode(root);
  const code = el('<div class="codecol"></div>');
  if (opts.progress) code.classList.add("has-progress");
  if (opts.boundaryTargets) code.classList.add("boundary-targets");
  root.appendChild(code);
  const addBoundary = (
    boundaryIndex?: number,
    selectable: boolean = false,
  ) => {
    const node = el('<div class="boundary"></div>');
    if (selectable) {
      node.classList.add("selectable");
      if (typeof boundaryIndex === "number") {
        node.dataset.boundary = String(boundaryIndex);
        const selected =
          opts.selectedBoundary != null &&
          Number(opts.selectedBoundary) === boundaryIndex;
        if (selected) node.classList.add("selected");
      }
    }
    code.appendChild(node);
  };
  const hideBoundary = !!opts.hideBoundary;
  const selectableBoundaries = Array.isArray(opts.selectableBoundaries)
    ? new Set(opts.selectableBoundaries)
    : null;
  const progress = !!opts.progress;
  let progressIndex = -1;
  let progressRangeStart: number | null = null;
  let progressRangeEnd: number | null = null;
  let strikeRanges: Array<[number, number]> = [];
  const strikeFragmentsByLine = new Map<
    number,
    Array<{ start: number; end: number }>
  >();
  let doneBoundary = boundary;
  if (typeof opts.doneBoundary === "number") {
    doneBoundary = Math.max(0, Math.min(lines.length, opts.doneBoundary));
  }
  if (progress) {
    const range = opts.progressRange;
    const maxIndex = Math.max(0, lines.length - 1);
    const progressRange = range ? normalizeLineRange(range, maxIndex) : null;
    if (progressRange) {
      progressRangeStart = progressRange[0];
      progressRangeEnd = progressRange[1];
      if (typeof opts.progressIndex === "number") {
        progressIndex = clampLineIndex(opts.progressIndex, lines.length - 1);
      } else if (!opts.suppressProgressMid && progressRangeStart != null) {
        progressIndex = progressRangeStart;
      }
    } else if (typeof opts.progressIndex === "number") {
      progressIndex = Math.max(
        0,
        Math.min(lines.length - 1, opts.progressIndex),
      );
    } else if (!opts.suppressProgressMid && boundary > 0) {
      progressIndex = boundary - 1;
    }
  }
  const appendStrike = (range: LineRange) => {
    const normalized = normalizeLineRange(range, Math.max(0, lines.length - 1));
    strikeRanges.push(normalized);
  };
  if (opts.strikeRange) appendStrike(opts.strikeRange);
  if (Array.isArray(opts.strikeRanges)) {
    opts.strikeRanges.forEach((range) => appendStrike(range));
  }
  if (Array.isArray(opts.strikeFragments)) {
    opts.strikeFragments.forEach((frag) => {
      const line = clampLineIndex(frag.line, lines.length - 1);
      const text = lines[line] ?? "";
      const max = text.length;
      let start = Math.max(0, Math.min(max, Number(frag.start)));
      let end = Math.max(0, Math.min(max, Number(frag.end)));
      if (end < start) [start, end] = [end, start];
      if (end <= start) return;
      const list = strikeFragmentsByLine.get(line) || [];
      list.push({ start, end });
      strikeFragmentsByLine.set(line, list);
    });
  }
  if (doneBoundary === 0 && !hideBoundary) addBoundary();
  if (selectableBoundaries && selectableBoundaries.has(0)) {
    addBoundary(0, true);
  }
  for (let i = 0; i < lines.length; i++) {
    const lr = el('<div class="line"></div>');
    const ln = el(`<div class="ln">${i + 1}</div>`);
    const src = el('<div class="src"></div>');
    const rawLine = lines[i];
    const inStrikeRange = strikeRanges.some(
      ([start, end]) => i >= start && i <= end,
    );
    const fragments = (strikeFragmentsByLine.get(i) || []).slice();
    if (inStrikeRange && rawLine.length > 0) {
      fragments.push({ start: 0, end: rawLine.length });
    }
    const mergedFragments: Array<{ start: number; end: number }> = [];
    if (fragments.length) {
      const sorted = fragments
        .slice()
        .sort((a, b) => a.start - b.start || a.end - b.end);
      sorted.forEach(({ start, end }) => {
        if (!mergedFragments.length) {
          mergedFragments.push({ start, end });
          return;
        }
        const last = mergedFragments[mergedFragments.length - 1]!;
        if (start <= last.end) {
          last.end = Math.max(last.end, end);
          return;
        }
        mergedFragments.push({ start, end });
      });
    }
    const hasFragments = mergedFragments.length > 0;
    if (hasFragments) {
      let cursor = 0;
      mergedFragments.forEach(({ start, end }) => {
        if (start > cursor) {
          src.appendChild(
            document.createTextNode(rawLine.slice(cursor, start)),
          );
        }
        if (end > start) {
          const span = document.createElement("span");
          span.className = "skipped-fragment";
          span.textContent = rawLine.slice(start, end);
          src.appendChild(span);
        }
        cursor = Math.max(cursor, end);
      });
      if (cursor < rawLine.length) {
        src.appendChild(document.createTextNode(rawLine.slice(cursor)));
      }
    } else {
      src.textContent = rawLine;
    }
    if (i < doneBoundary) lr.classList.add("done");
    const inProgressRange =
      progressRangeStart !== null &&
      progressRangeEnd !== null &&
      i >= progressRangeStart &&
      i <= progressRangeEnd;
    if (inProgressRange) lr.classList.add("progress-range");
    if (inStrikeRange && !hasFragments) lr.classList.add("skipped");
    if (i === progressIndex) lr.classList.add("progress-mid");
    lr.appendChild(ln);
    lr.appendChild(src);
    code.appendChild(lr);
    const afterLine = i + 1;
    if (selectableBoundaries && selectableBoundaries.has(afterLine)) {
      addBoundary(afterLine, true);
    }
    if (!hideBoundary && i + 1 === doneBoundary && i !== progressIndex) {
      addBoundary();
    }
  }
}

function vbox({
  address = "—",
  type = "int",
  value = "",
  name = "",
  editable = false,
  allowNameEdit = false,
  allowTypeEdit = false,
  showDoubleExact = false,
}: {
  address?: string;
  type?: string;
  value?: BoxValue;
  name?: string;
  editable?: boolean;
  allowNameEdit?: boolean;
  allowTypeEdit?: boolean;
  showDoubleExact?: boolean;
} = {}): HTMLElement {
  const parsedType = parseType(type || "int");
  const isFloatingScalar =
    (parsedType.base === "float" || parsedType.base === "double") &&
    parsedType.depth === 0;
  const rawValue = value ?? "";
  const emptyDisplay = rawValue === "";
  const displayValue = emptyDisplay ? "" : normalizeZeroDisplay(rawValue);
  const resolvedName = name ?? "";
  const namesList = resolvedName ? [resolvedName] : [""];
  const valueClasses = `value ${editable ? "editable" : ""} ${emptyDisplay ? "placeholder muted" : ""}`;
  const typeClasses = `type ${allowTypeEdit ? "editable" : ""}`;
  const nameClasses = `name-tag ${editable ? "editable" : ""}`;
  const listClasses = "name-list";
  const nameTags = namesList
    .map((n) => {
      const cls =
        namesList.length > 1 ? `${nameClasses}` : `${nameClasses} single`;
      return `<span class="${cls}"><span class="name-text">${n}</span></span>`;
    })
    .join("");
  const namesHtml = nameTags;

  const node = el(`
    <div class="vbox ${editable ? "is-editable" : ""}">
      <div class="vbox-main">
        <div class="vbox-address-row">
          <div class="lbl lbl-addr">address</div>
          <div class="address">${address}</div>
        </div>
        <div class="cell">
          <div class="lbl lbl-value">value</div>
          <div class="${valueClasses}">${displayValue}</div>
          <button class="double-toggle hidden" type="button" aria-pressed="false">exact</button>
        </div>
        <div class="name-stack">
          <div class="${listClasses}">
            <div class="name-list-inner">${namesHtml}</div>
          </div>
          <div class="lbl lbl-name">${namesList.length > 1 ? "name(s)" : "name"}</div>
        </div>
      </div>
      <div class="vbox-meta">
        <div class="lbl lbl-type">type</div>
        <div class="${typeClasses}">${type}</div>
      </div>
    </div>
  `);

  const valueEl = node.querySelector(".value") as HTMLElement | null;

  if (valueEl) {
    valueEl.dataset.rawValue = rawValue.trim();
  }

  if (editable && valueEl) {
    valueEl.setAttribute("contenteditable", "true");
    disableAutoText(valueEl);
    valueEl.addEventListener("input", () => {
      const raw = valueEl.textContent || "";
      const text = raw.replace(/\s+/g, "");
      valueEl.dataset.rawValue = raw.trim();
      if (!text) {
        valueEl.classList.add("placeholder", "muted");
        valueEl.dataset.empty = "true";
        valueEl.textContent = "";
      } else {
        valueEl.classList.remove("placeholder", "muted");
        delete valueEl.dataset.empty;
      }
    });
    if (emptyDisplay) {
      valueEl.dataset.empty = "true";
    }
    if (allowTypeEdit) {
      const typeEl = node.querySelector(".type") as HTMLElement | null;
      if (typeEl) {
        typeEl.setAttribute("contenteditable", "true");
        typeEl.classList.add("editable");
        disableAutoText(typeEl);
      }
    }
    node.querySelectorAll(".name-text").forEach((el) => {
      if (!allowNameEdit || !(el instanceof HTMLElement)) return;
      el.setAttribute("contenteditable", "true");
      el.classList.add("editable");
      disableAutoText(el);
    });
  }
  const toggleEl = node.querySelector(
    ".double-toggle",
  ) as HTMLButtonElement | null;
  if (valueEl && isFloatingScalar && !emptyDisplay && !editable) {
    const parsed = parseDoubleValueWithSign(rawValue);
    if (parsed != null) {
      const nanSign = parsed.nanSign;
      valueEl.dataset.doubleValue = formatDoubleStorage(parsed.value, nanSign);
      node.dataset.doubleDisplay = showDoubleExact ? "exact" : "short";
      const exactText = formatDoubleExact(parsed.value, nanSign);
      const defaultText = formatDoubleDefault(parsed.value, nanSign);
      const approx = !doubleDisplayIsExact(defaultText, exactText);
      valueEl.textContent = showDoubleExact ? exactText : defaultText;
      if (approx && !showDoubleExact) valueEl.dataset.doubleApprox = "true";
      else delete valueEl.dataset.doubleApprox;
      if (toggleEl) {
        toggleEl.textContent = showDoubleExact ? "short" : "exact";
        toggleEl.setAttribute(
          "aria-pressed",
          showDoubleExact ? "true" : "false",
        );
        if (approx) {
          toggleEl.classList.remove("hidden");
        } else {
          toggleEl.classList.add("hidden");
        }
        toggleEl.addEventListener("click", (e) => {
          e.preventDefault();
          e.stopPropagation();
          const currentValue = parseDoubleValueWithSign(
            valueEl.dataset.doubleValue ?? valueEl.textContent ?? "",
          );
          if (currentValue == null) return;
          const nextNanSign = currentValue.nanSign;
          const nextExact = node.dataset.doubleDisplay !== "exact";
          const nextExactText = formatDoubleExact(
            currentValue.value,
            nextNanSign,
          );
          const nextDefaultText = formatDoubleDefault(
            currentValue.value,
            nextNanSign,
          );
          const nextApprox = !doubleDisplayIsExact(
            nextDefaultText,
            nextExactText,
          );
          node.dataset.doubleDisplay = nextExact ? "exact" : "short";
          valueEl.textContent = nextExact ? nextExactText : nextDefaultText;
          if (nextApprox && !nextExact) valueEl.dataset.doubleApprox = "true";
          else delete valueEl.dataset.doubleApprox;
          toggleEl.textContent = nextExact ? "short" : "exact";
          toggleEl.setAttribute("aria-pressed", nextExact ? "true" : "false");
          if (nextApprox) {
            toggleEl.classList.remove("hidden");
          } else {
            toggleEl.classList.add("hidden");
          }
        });
      }
    }
  } else if (toggleEl) {
    toggleEl.classList.add("hidden");
  }
  return node;
}

function disableBoxEditing(root: Element | null) {
  if (!root) return;
  root
    .querySelectorAll(
      ".value.editable, .type.editable, .name-text.editable, .array-col-value.editable",
    )
    .forEach((el) => {
      el.removeAttribute("contenteditable");
      el.classList.remove("editable");
    });
  root.classList.remove("is-editable");
}

function removeBoxDeleteButtons(root?: Element | null) {
  const scope = root || document;
  scope.querySelectorAll(".vbox .delete, .arraybox .delete").forEach((btn) => btn.remove());
}

function readBoxState(root: Element): BoxState {
  const el = root as HTMLElement;
  const names = [...root.querySelectorAll(".name-text")]
    .map((el) => txt(el))
    .filter(Boolean);
  const valEl = root.querySelector(".value");
  const valText = txt(valEl);
  const typeText = txt(root.querySelector(".type"));
  const parsedType = parseType(typeText || "int");
  const placeholderEmpty =
    valEl?.classList?.contains("placeholder") && valText === "";
  const fallbackRawValue = placeholderEmpty ? "" : valText;
  const storedRawValue =
    valEl instanceof HTMLElement ? valEl.dataset.rawValue : undefined;
  const rawValue =
    storedRawValue !== undefined ? storedRawValue : fallbackRawValue;
  let value = normalizeZeroDisplay(rawValue);
  const valueEditable =
    valEl instanceof HTMLElement && valEl.isContentEditable;
  if (
    (parsedType.base === "float" || parsedType.base === "double") &&
    parsedType.depth === 0 &&
    valEl instanceof HTMLElement &&
    !valueEditable
  ) {
    const stored = valEl.dataset.doubleValue;
    if (stored != null && stored !== "") value = stored;
  }
  const allowDelete =
    el.dataset.allowDelete === "true" || !!root.querySelector(".delete");
  return {
    address: txt(root.querySelector(".address")),
    type: typeText,
    value,
    rawValue,
    name: names[0] || "",
    names,
    allowNameEdit: !!root.querySelector(".name-text[contenteditable]"),
    allowTypeEdit: !!root.querySelector(".type[contenteditable]"),
    allowDelete,
    showDoubleExact: el.dataset.doubleDisplay === "exact",
  };
}

function boxAddress(box: BoxState | null | undefined): string {
  const raw = box?.address ?? "";
  return raw.trim();
}

function collectStageBoxes(root: Element): BoxState[] {
  return [...root.querySelectorAll(".vbox")]
    .map((node) => {
      const box = readBoxState(node);
      box.node = node as HTMLElement;
      return box;
    });
}

function buildOtherNamesMap(boxes: BoxState[]): Map<string, Set<string>> {
  const byAddr = new Map<string, BoxState>();
  boxes.forEach((box) => {
    const addr = boxAddress(box);
    if (addr) byAddr.set(addr, box);
  });
  const otherNamesByAddr = new Map<string, Set<string>>();
  boxes.forEach((box) => {
    const baseName = String(box.name || "").trim();
    const depth = Math.max(0, Math.floor(getPointerDepth(box.type)));
    if (!baseName || depth < 1) return;
    let pointedAddr = String(box.value ?? "").trim();
    for (let step = 1; step <= depth; step += 1) {
      if (!pointedAddr) break;
      const target = byAddr.get(pointedAddr);
      if (!target) break;
      const resolvedAddr = boxAddress(target);
      if (resolvedAddr) {
        let bucket = otherNamesByAddr.get(resolvedAddr);
        if (!bucket) {
          bucket = new Set<string>();
          otherNamesByAddr.set(resolvedAddr, bucket);
        }
        bucket.add(`${"*".repeat(step)}${baseName}`);
      }
      pointedAddr = String(target.value ?? "").trim();
    }
  });
  return otherNamesByAddr;
}

function sortOtherNames(list: Iterable<string>): string[] {
  return [...list].sort((a, b) => {
    const aStars = (a.match(/^\*+/) || [""])[0].length;
    const bStars = (b.match(/^\*+/) || [""])[0].length;
    if (aStars !== bStars) return aStars - bStars;
    return a.localeCompare(b);
  });
}

function updateOtherNamesList(
  node: HTMLElement,
  aliases: string[],
  showAliases: boolean,
): void {
  const listInner = node.querySelector(".name-list-inner");
  if (!listInner) return;
  const tags = listInner.querySelectorAll(".name-tag");
  let baseTag = tags[0];
  if (!baseTag) {
    baseTag = document.createElement("span");
    baseTag.className = "name-tag single";
    const text = document.createElement("span");
    text.className = "name-text";
    baseTag.appendChild(text);
    listInner.appendChild(baseTag);
  }
  [...listInner.children].slice(1).forEach((child) => child.remove());
  const hasAliases =
    showAliases && aliases.length > 0;
  baseTag.classList.toggle("single", !hasAliases);
  if (hasAliases) {
    aliases.forEach((alias) => {
      const tag = document.createElement("span");
      tag.className = "name-tag name-tag-derived";
      const text = document.createElement("span");
      text.className = "name-text";
      text.textContent = alias;
      tag.appendChild(text);
      listInner.appendChild(tag);
    });
  }
  const label = node.querySelector(".lbl-name");
  if (label) label.textContent = hasAliases ? "names" : "name";
}

function ensureOtherNamesToggle(
  node: HTMLElement,
  onToggle: (target: HTMLElement) => void,
): HTMLButtonElement | null {
  const stack = node.querySelector(".name-stack");
  const label = node.querySelector(".lbl-name");
  if (!stack || !label) return null;
  let btn = node.querySelector(
    ".other-names-toggle",
  ) as HTMLButtonElement | null;
  if (!btn) {
    btn = document.createElement("button");
    btn.type = "button";
    btn.className = "other-names-toggle";
    btn.textContent = "Show aliases";
  }
  if (!btn.dataset.bound) {
    btn.dataset.bound = "true";
    btn.addEventListener("click", (event: MouseEvent) => {
      event.preventDefault();
      onToggle(node);
    });
  }
  if (btn.parentElement !== stack || btn.nextElementSibling !== label) {
    stack.insertBefore(btn, label);
  }
  return btn;
}

function applyOtherNames(
  root: Element | null,
  opts: {
    onToggle?: (() => void) | null;
    shownAddrs?: Set<string> | null;
    sourceBoxes?: BoxState[] | null;
    cleanupShownAddrs?: boolean;
  } = {},
) {
  if (!root) return;
  const {
    onToggle = null,
    shownAddrs = null,
    sourceBoxes = null,
    cleanupShownAddrs = true,
  } = opts;
  const boxes = collectStageBoxes(root);
  const currentAddrs = new Set(boxes.map((box) => boxAddress(box)));
  const aliasSource =
    Array.isArray(sourceBoxes) && sourceBoxes.length ? sourceBoxes : boxes;
  const otherNamesByAddr = buildOtherNamesMap(aliasSource);
  const useShownSet = shownAddrs !== null;
  const getShown = (node: HTMLElement, addr: string) =>
    useShownSet ? shownAddrs.has(addr) : node.dataset.otherNames === "on";
  const setShown = (node: HTMLElement, addr: string, value: boolean) => {
    if (useShownSet) {
      if (value) shownAddrs.add(addr);
      else shownAddrs.delete(addr);
    }
    node.dataset.otherNames = value ? "on" : "off";
  };
  boxes.forEach((box) => {
    const node = box.node;
    if (!node) return;
    const addr = boxAddress(box);
    const baseName = String(box.name || "").trim();
    const otherNames = otherNamesByAddr.get(addr);
    const aliases = otherNames ? sortOtherNames(otherNames) : [];
    const filtered = baseName
      ? aliases.filter((alias) => alias !== baseName)
      : aliases;
    node.dataset.otherNamesAddr = addr;
    if (!filtered.length) {
      const toggle = node.querySelector(".other-names-toggle");
      if (toggle) {
        toggle.textContent = "Show aliases";
        toggle.classList.add("hidden");
        toggle.setAttribute("aria-pressed", "false");
      }
      setShown(node, addr, false);
      updateOtherNamesList(node, [], false);
      return;
    }
    const toggle = ensureOtherNamesToggle(node, (targetNode) => {
      const targetAddr = targetNode.dataset.otherNamesAddr || addr;
      const next = !getShown(targetNode, targetAddr);
      setShown(targetNode, targetAddr, next);
      onToggle?.();
    });
    if (toggle) {
      toggle.classList.remove("hidden");
    }
    const showAliases = getShown(node, addr);
    if (toggle) {
      toggle.textContent = showAliases ? "Hide aliases" : "Show aliases";
      toggle.setAttribute("aria-pressed", showAliases ? "true" : "false");
    }
    updateOtherNamesList(node, filtered, showAliases);
  });
  if (useShownSet && cleanupShownAddrs) {
    shownAddrs.forEach((addr) => {
      if (!currentAddrs.has(addr)) shownAddrs.delete(addr);
    });
  }
}

const addrPool: { free: string[] } = { free: [] };
function nextPooledAddr(type: string = "int") {
  if (addrPool.free.length) return addrPool.free.pop();
  return randAddr(type);
}

function makeAnswerBox({
  name = "",
  type = "",
  value = "",
  address = null,
  editable = true,
  deletable = editable,
  allowNameEdit = null,
  allowTypeEdit = null,
  showDoubleExact = null,
}: {
  name?: string;
  type?: string;
  value?: BoxValue;
  address?: string | number | null;
  editable?: boolean;
  deletable?: boolean;
  allowNameEdit?: boolean | null;
  allowTypeEdit?: boolean | null;
  showDoubleExact?: boolean | null;
} = {}) {
  const resolvedAddr =
    address == null ? String(nextPooledAddr(type || "int")) : String(address);
  const resolvedNameEdit =
    allowNameEdit !== null && allowNameEdit !== undefined ? allowNameEdit : !name;
  const resolvedTypeEdit =
    allowTypeEdit !== null && allowTypeEdit !== undefined ? allowTypeEdit : !type;
  const node = vbox({
    address: resolvedAddr,
    type,
    value,
    name,
    editable,
    allowNameEdit: resolvedNameEdit,
    allowTypeEdit: resolvedTypeEdit,
    showDoubleExact: showDoubleExact ?? false,
  });
  if (deletable) {
    const del = el('<button class="delete" title="delete">×</button>');
    node.appendChild(del);
    del.onclick = () => {
      const addrTxt = txt(node.querySelector(".address"));
      if (addrTxt) addrPool.free.push(addrTxt);
      node.remove();
    };
  }
  return node;
}

type GroupedScalarObject = {
  kind: "scalar";
  box: BoxState;
  index: number;
};

type GroupedArrayElement = {
  box: BoxState;
  indices: number[];
  shape: number[];
  index: number;
  linear: number;
};

type GroupedArrayObject = {
  kind: "array";
  name: string;
  shape: number[];
  index: number;
  elementType: string;
  entries: GroupedArrayElement[];
  allowDelete: boolean;
};

type GroupedStateObject = GroupedScalarObject | GroupedArrayObject;

type EvaluatedExpressionResultLike = {
  kind?: string;
  type?: string;
  value?: BoxValue | bigint | number;
  address?: string;
};

function normalizeArrayDims(value: unknown): number[] {
  if (!Array.isArray(value)) return [];
  const out: number[] = [];
  for (const raw of value) {
    const num = Math.floor(Number(raw));
    if (!Number.isFinite(num) || num <= 0) return [];
    out.push(num);
  }
  return out;
}

function parseArrayElementName(name: string): { baseName: string; indices: number[] } | null {
  const match = /^([A-Za-z_][A-Za-z0-9_]*)((?:\[\d+\])+)$/.exec(
    String(name || ""),
  );
  if (!match) return null;
  const baseName = match[1] || "";
  const suffix = match[2] || "";
  if (!baseName || !suffix) return null;
  const indices: number[] = [];
  const rx = /\[(\d+)\]/g;
  let m: RegExpExecArray | null;
  while ((m = rx.exec(suffix)) != null) {
    const value = Number(m[1]);
    if (!Number.isFinite(value) || value < 0) return null;
    indices.push(value);
  }
  if (!indices.length) return null;
  return { baseName, indices };
}

function arrayElementName(name: string, indices: number[]): string {
  return `${name}${indices.map((index) => `[${index}]`).join("")}`;
}

function inferArrayShapeFromEntries(
  entries: Array<{ indices: number[] }>,
): number[] {
  if (!entries.length) return [];
  const dims = entries[0]?.indices.length || 0;
  if (!dims) return [];
  const shape = new Array(dims).fill(0);
  for (const entry of entries) {
    if (entry.indices.length !== dims) return [];
    for (let i = 0; i < dims; i++) {
      const value = entry.indices[i]!;
      if (value < 0) return [];
      shape[i] = Math.max(shape[i]!, value + 1);
    }
  }
  return shape.every((dim) => dim > 0) ? shape : [];
}

function arrayLinearIndex(indices: number[], shape: number[]): number | null {
  if (indices.length !== shape.length) return null;
  let linear = 0;
  let stride = 1;
  for (let i = shape.length - 1; i >= 0; i--) {
    const idx = indices[i]!;
    const dim = shape[i]!;
    if (idx < 0 || idx >= dim) return null;
    linear += idx * stride;
    stride *= dim;
  }
  return linear;
}

function arrayElementCount(shape: number[]): number {
  return shape.reduce((acc, dim) => acc * dim, 1);
}

function sameDims(a: number[], b: number[]): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) {
    if (a[i] !== b[i]) return false;
  }
  return true;
}

function groupStateObjects(boxes: BoxState[]): GroupedStateObject[] {
  const arrayCandidates = new Map<
    string,
    Array<{ box: BoxState; indices: number[]; shape: number[]; index: number }>
  >();
  const scalars: GroupedScalarObject[] = [];
  boxes.forEach((box, index) => {
    let baseName = "";
    let indices: number[] = [];
    const metaShape = normalizeArrayDims(box.arrayShape);
    if (box.arrayRoot && Array.isArray(box.arrayIndices) && box.arrayIndices.length > 0) {
      baseName = String(box.arrayRoot);
      indices = box.arrayIndices
        .map((value) => Math.floor(Number(value)))
        .filter((value) => Number.isFinite(value) && value >= 0);
    } else {
      const parsed = parseArrayElementName(box.name || "");
      if (parsed) {
        baseName = parsed.baseName;
        indices = parsed.indices;
      }
    }
    if (!baseName || !indices.length) {
      scalars.push({ kind: "scalar", box, index });
      return;
    }
    const list = arrayCandidates.get(baseName) || [];
    list.push({ box, indices, shape: metaShape, index });
    arrayCandidates.set(baseName, list);
  });

  const out: GroupedStateObject[] = [...scalars];
  for (const [baseName, list] of arrayCandidates.entries()) {
    if (!list.length) continue;
    const sorted = list.slice().sort((a, b) => a.index - b.index);
    const first = sorted[0]!;
    const shapes = sorted
      .map((item) => item.shape)
      .filter((shape) => shape.length > 0);
    let shape = shapes.length ? shapes[0]!.slice() : inferArrayShapeFromEntries(sorted);
    if (shape.length && shapes.some((candidate) => !sameDims(candidate, shape))) {
      shape = [];
    }
    if (!shape.length) {
      sorted.forEach((item) => {
        out.push({ kind: "scalar", box: item.box, index: item.index });
      });
      continue;
    }
    const elementType = String(first.box.type || "").trim();
    if (!elementType || sorted.some((item) => String(item.box.type || "").trim() !== elementType)) {
      sorted.forEach((item) => {
        out.push({ kind: "scalar", box: item.box, index: item.index });
      });
      continue;
    }
    const entries: GroupedArrayElement[] = [];
    const seenLinear = new Set<number>();
    let valid = true;
    for (const item of sorted) {
      const linear = arrayLinearIndex(item.indices, shape);
      if (linear == null || seenLinear.has(linear)) {
        valid = false;
        break;
      }
      seenLinear.add(linear);
      entries.push({
        box: item.box,
        indices: item.indices.slice(),
        shape: shape.slice(),
        index: item.index,
        linear,
      });
    }
    if (!valid || seenLinear.size !== arrayElementCount(shape)) {
      sorted.forEach((item) => {
        out.push({ kind: "scalar", box: item.box, index: item.index });
      });
      continue;
    }
    entries.sort((a, b) => a.linear - b.linear);
    out.push({
      kind: "array",
      name: baseName,
      shape: shape.slice(),
      index: first.index,
      elementType,
      entries,
      allowDelete: entries.every((entry) => !!entry.box.allowDelete),
    });
  }
  out.sort((a, b) => a.index - b.index);
  return out;
}

function normalizeComparableType(typeText: string): string {
  return String(typeText || "")
    .replace(/\s+/g, "")
    .trim();
}

function makeSubarrayRootName(baseName: string, prefix: number[]): string {
  return `${baseName}${prefix.map((index) => `[${index}]`).join("")}`;
}

function buildSyntheticSubarrayBoxes(
  sourceEntries: GroupedArrayElement[],
  baseName: string,
  prefix: number[],
  shape: number[],
): BoxState[] {
  const subRoot = makeSubarrayRootName(baseName, prefix);
  return sourceEntries.map((entry) => {
    const suffix = entry.indices.slice(prefix.length);
    return {
      ...entry.box,
      name: arrayElementName(subRoot, suffix),
      arrayRoot: subRoot,
      arrayShape: shape.slice(),
      arrayIndices: suffix,
    };
  });
}

function findSubarrayBoxesInGroup(
  group: GroupedArrayObject,
  expectedShape: number[],
  baseAddress: string,
): BoxState[] | null {
  const rank = group.shape.length;
  if (!rank || !expectedShape.length || expectedShape.length > rank) return null;
  if (expectedShape.length === rank) {
    if (!sameDims(group.shape, expectedShape)) return null;
    const firstAddr = String(group.entries[0]?.box.address ?? "").trim();
    if (firstAddr !== baseAddress) return null;
    return group.entries.map((entry) => entry.box);
  }
  const prefixLen = rank - expectedShape.length;
  if (!sameDims(group.shape.slice(prefixLen), expectedShape)) return null;
  const start = group.entries.find((entry) => {
    if (entry.indices.length !== rank) return false;
    if (String(entry.box.address ?? "").trim() !== baseAddress) return false;
    for (let i = prefixLen; i < rank; i++) {
      if ((entry.indices[i] ?? -1) !== 0) return false;
    }
    return true;
  });
  if (!start) return null;
  const prefix = start.indices.slice(0, prefixLen);
  const subset = group.entries.filter((entry) => {
    if (entry.indices.length !== rank) return false;
    for (let i = 0; i < prefixLen; i++) {
      if (entry.indices[i] !== prefix[i]) return false;
    }
    return true;
  });
  if (subset.length !== arrayElementCount(expectedShape)) return null;
  const rebased = subset.map((entry) => ({
    entry,
    suffix: entry.indices.slice(prefixLen),
  }));
  const seen = new Set<number>();
  for (const item of rebased) {
    const linear = arrayLinearIndex(item.suffix, expectedShape);
    if (linear == null || seen.has(linear)) return null;
    seen.add(linear);
  }
  if (seen.size !== arrayElementCount(expectedShape)) return null;
  const sortedSubset = rebased
    .slice()
    .sort(
      (left, right) =>
        (arrayLinearIndex(left.suffix, expectedShape) || 0) -
        (arrayLinearIndex(right.suffix, expectedShape) || 0),
    )
    .map((item) => item.entry);
  return buildSyntheticSubarrayBoxes(
    sortedSubset,
    group.name,
    prefix,
    expectedShape,
  );
}

function findArrayObjectBoxesForResult(
  result: EvaluatedExpressionResultLike | null | undefined,
  state: BoxState[] | null | undefined,
): BoxState[] | null {
  if (!result || !Array.isArray(state) || !state.length) return null;
  const parsed = parseType(String(result.type || "int"));
  const expectedShape = normalizeArrayDims(parsed.arrayDims);
  if (!expectedShape.length) return null;
  const baseAddress = String(result.value ?? "").trim();
  if (!baseAddress) return null;
  const grouped = groupStateObjects(state);
  const expectedType = normalizeComparableType(String(result.type || ""));
  for (const item of grouped) {
    if (item.kind !== "array") continue;
    const fullType = normalizeComparableType(
      `${item.elementType}${item.shape.map((d) => `[${d}]`).join("")}`,
    );
    const isRootTypeMatch = fullType === expectedType;
    if (isRootTypeMatch) {
      const firstAddr = String(item.entries[0]?.box.address ?? "").trim();
      if (firstAddr === baseAddress) {
        return item.entries.map((entry) => entry.box);
      }
    }
    const subset = findSubarrayBoxesInGroup(item, expectedShape, baseAddress);
    if (subset) return subset;
  }
  return null;
}

function makeArrayBox(
  group: GroupedArrayObject,
  opts: {
    editable: boolean;
    deletable: boolean;
  },
): HTMLElement {
  const { editable, deletable } = opts;
  const typeText = `${group.elementType}${group.shape.map((d) => `[${d}]`).join("")}`;
  const firstAddress = String(group.entries[0]?.box.address ?? "—");
  const node = el(`
    <div class="arraybox ${editable ? "is-editable" : ""}">
      <div class="arraybox-main">
        <div class="arraybox-address-row">
          <div class="lbl lbl-array-addr">address</div>
          <div class="array-address"></div>
        </div>
        <div class="array-values-wrap">
          <div class="array-label array-values-label">values</div>
          <div class="array-values"></div>
        </div>
        <div class="array-name-stack">
          <div class="array-name"></div>
          <div class="lbl lbl-array-name">name</div>
        </div>
      </div>
      <div class="arraybox-meta">
        <div class="lbl lbl-array-type">type</div>
        <div class="array-type"></div>
      </div>
    </div>
  `);
  node.dataset.arrayRoot = group.name;
  node.dataset.arrayShape = group.shape.join(",");
  node.dataset.arrayElementType = group.elementType;
  node.querySelector(".array-address")!.textContent = firstAddress;
  node.querySelector(".array-type")!.textContent = typeText;
  node.querySelector(".array-name")!.textContent = group.name;

  const valuesWrap = node.querySelector(".array-values");
  if (!valuesWrap) return node;
  const lastDim = group.shape[group.shape.length - 1]!;
  let currentRowKey = "";
  let rowEl: HTMLElement | null = null;
  let rowValuesEl: HTMLElement | null = null;
  for (const entry of group.entries) {
    const prefix = entry.indices.slice(0, -1);
    const rowKey = prefix.join(",");
    if (!rowEl || rowKey !== currentRowKey) {
      rowEl = document.createElement("div");
      rowEl.className = "array-row";
      if (group.shape.length > 1) {
        const rowLabel = document.createElement("div");
        rowLabel.className = "array-row-label";
        rowLabel.textContent = prefix.map((value) => `[${value}]`).join("");
        rowEl.appendChild(rowLabel);
      }
      rowValuesEl = document.createElement("div");
      rowValuesEl.className = "array-row-values";
      rowValuesEl.style.gridTemplateColumns = `repeat(${lastDim}, minmax(64px, 1fr))`;
      rowEl.appendChild(rowValuesEl);
      valuesWrap.appendChild(rowEl);
      currentRowKey = rowKey;
    }
    if (!rowValuesEl) continue;
    const col = document.createElement("div");
    col.className = "array-col";
    const valueEl = document.createElement("div");
    valueEl.className = "array-col-value";
    const raw = entry.box.rawValue ?? entry.box.value ?? "";
    const empty = raw === "";
    valueEl.textContent = empty ? "" : normalizeZeroDisplay(raw);
    valueEl.dataset.rawValue = String(raw).trim();
    valueEl.dataset.arrayName = entry.box.name;
    valueEl.dataset.arrayType = entry.box.type;
    valueEl.dataset.arrayAddress = String(entry.box.address ?? "");
    valueEl.dataset.arrayIndices = entry.indices.join(",");
    if (empty) valueEl.classList.add("placeholder", "muted");
    if (editable) {
      valueEl.setAttribute("contenteditable", "true");
      valueEl.classList.add("editable");
      disableAutoText(valueEl);
      valueEl.addEventListener("input", () => {
        const rawText = valueEl.textContent || "";
        const compact = rawText.replace(/\s+/g, "");
        valueEl.dataset.rawValue = rawText.trim();
        if (!compact) {
          valueEl.textContent = "";
          valueEl.classList.add("placeholder", "muted");
        } else {
          valueEl.classList.remove("placeholder", "muted");
        }
      });
    }
    col.appendChild(valueEl);
    rowValuesEl.appendChild(col);
  }
  if (deletable && group.allowDelete) {
    const del = el('<button class="delete" title="delete">×</button>');
    node.appendChild(del);
    del.addEventListener("click", () => {
      group.entries.forEach((entry) => {
        const addr = String(entry.box.address ?? "").trim();
        if (addr) addrPool.free.push(addr);
      });
      node.remove();
    });
    node.dataset.allowDelete = "true";
  }
  return node;
}

function appendStateObjects(
  container: HTMLElement,
  boxes: BoxState[],
  opts: {
    editable?: boolean;
    deletable?: boolean;
    allowNameEdit?: boolean | null;
    allowTypeEdit?: boolean | null;
  } = {},
): void {
  const {
    editable = false,
    deletable = editable,
    allowNameEdit = null,
    allowTypeEdit = null,
  } = opts;
  const grouped = groupStateObjects(Array.isArray(boxes) ? boxes : []);
  grouped.forEach((item) => {
    if (item.kind === "scalar") {
      const box = item.box;
      const allowDelete =
        box.allowDelete !== null && box.allowDelete !== undefined
          ? !!box.allowDelete
          : deletable;
      const node = makeAnswerBox({
        name: box.name,
        type: box.type,
        value: box.rawValue ?? box.value,
        address: box.address ?? null,
        editable,
        deletable: allowDelete,
        allowNameEdit: allowNameEdit ?? box.allowNameEdit,
        allowTypeEdit: allowTypeEdit ?? box.allowTypeEdit,
        showDoubleExact: box.showDoubleExact ?? null,
      });
      if (allowDelete) node.dataset.allowDelete = "true";
      if ((box.value ?? "") === "") {
        node.querySelector(".value")?.classList.add("placeholder", "muted");
      }
      container.appendChild(node);
      return;
    }
    const node = makeArrayBox(item, {
      editable,
      deletable,
    });
    container.appendChild(node);
  });
}

function readArrayBoxState(root: HTMLElement): BoxState[] {
  const shape = normalizeArrayDims(
    String(root.dataset.arrayShape || "")
      .split(",")
      .filter(Boolean)
      .map((value) => Number(value)),
  );
  const rootName = String(root.dataset.arrayRoot || "").trim();
  const values = [...root.querySelectorAll(".array-col-value")];
  return values.map((valueNode) => {
    const valueEl = valueNode as HTMLElement;
    const typeText = String(
      valueEl.dataset.arrayType || root.dataset.arrayElementType || "int",
    ).trim();
    const valText = txt(valueEl);
    const placeholderEmpty =
      valueEl.classList.contains("placeholder") && valText === "";
    const storedRawValue = valueEl.dataset.rawValue;
    const fallbackRawValue = placeholderEmpty ? "" : valText;
    const rawValue =
      storedRawValue !== undefined ? storedRawValue : fallbackRawValue;
    const value = normalizeZeroDisplay(rawValue);
    const indices = String(valueEl.dataset.arrayIndices || "")
      .split(",")
      .filter(Boolean)
      .map((num) => Math.floor(Number(num)))
      .filter((num) => Number.isFinite(num) && num >= 0);
    const fallbackName =
      rootName && indices.length ? arrayElementName(rootName, indices) : "";
    return {
      address: String(valueEl.dataset.arrayAddress || "").trim(),
      type: typeText,
      value,
      rawValue,
      name: String(valueEl.dataset.arrayName || fallbackName).trim(),
      names: [],
      arrayRoot: rootName || null,
      arrayShape: shape.length ? shape.slice() : null,
      arrayIndices: indices.length ? indices.slice() : null,
      allowDelete:
        root.dataset.allowDelete === "true" ||
        (root.querySelector(".delete") != null),
    } as BoxState;
  });
}

function serializeWorkspace(
  target: string | Element | null,
): BoxState[] | null {
  if (!target) return null;
  let ws: Element | null = null;
  if (typeof target === "string") {
    ws = document.getElementById(target);
    if (!ws) return null;
  } else {
    ws = target;
  }
  if (!ws) return null;
  const wsEl = ws as HTMLElement;
  let nodes = [...ws.querySelectorAll(".vbox, .arraybox")];
  if (!nodes.length && wsEl.dataset.inline === "true") {
    const key = wsEl.dataset.workspaceKey || "";
    if (key) {
      nodes = [
        ...document.querySelectorAll(
          `.vbox[data-workspace="${key}"], .arraybox[data-workspace="${key}"]`,
        ),
      ];
    }
  }
  const out: BoxState[] = [];
  nodes.forEach((node) => {
    const el = node as HTMLElement;
    if (el.classList.contains("arraybox")) {
      out.push(...readArrayBoxState(el));
      return;
    }
    out.push(readBoxState(el));
  });
  return out;
}

function restoreWorkspace(
  state?: BoxState[] | null,
  defaults?: BoxState[] | null,
  opts: {
    editable?: boolean;
    deletable?: boolean;
    allowNameEdit?: boolean | null;
    allowTypeEdit?: boolean | null;
  } = {},
) {
  const {
    editable = true,
    deletable = editable,
    allowNameEdit = null,
    allowTypeEdit = null,
  } = opts;
  const wrap = el('<div class="grid" data-role="workspace"></div>');
  const source = Array.isArray(state) && state.length ? state : defaults || [];
  appendStateObjects(wrap, source, {
    editable,
    deletable,
    allowNameEdit,
    allowTypeEdit,
  });
  return wrap;
}

type StyledToken = { kind: "tok"; role: string; text: string };
type StyledSegment = string | StyledToken;

function parseStyledText(text: string): StyledSegment[] {
  const map: Record<string, string> = {
    n: "name",
    t: "type",
    v: "value",
    a: "addr",
    c: "code",
    b: "btn",
    i: "italic",
  };
  const raw = text;
  const out: StyledSegment[] = [];
  let i = 0;
  while (i < raw.length) {
    const idx = raw.indexOf("$", i);
    if (idx < 0) {
      out.push(raw.slice(i));
      break;
    }
    if (idx > i) out.push(raw.slice(i, idx));
    const key = raw[idx + 1];
    if (map[key] && raw[idx + 2] === "{") {
      let j = idx + 3;
      let foundEnd = false;
      let text = "";
      while (j < raw.length) {
        const ch = raw[j];
        if (ch === "\\" && j + 1 < raw.length) {
          const next = raw[j + 1];
          if (next === "{" || next === "}") {
            text += next;
            j += 2;
            continue;
          }
        }
        if (ch === "}") {
          foundEnd = true;
          break;
        }
        text += ch;
        j += 1;
      }
      if (foundEnd) {
        out.push({
          kind: "tok",
          role: map[key],
          text,
        });
        i = j + 1;
        continue;
      }
    }
    out.push("$");
    i = idx + 1;
  }
  return out;
}

type RenderPart = string | number | TokenPart | { kind: "br" };
type RenderParts = RenderPart | RenderPart[];

function renderParts(panel: Element | null, parts: RenderParts) {
  if (!panel) return;
  clearNode(panel);
  const list = Array.isArray(parts) ? parts : [parts];
  const appendText = (value: string | number) => {
    const text = String(value);
    const chunks = text.split("\n");
    chunks.forEach((chunk, idx) => {
      if (idx > 0) panel.appendChild(document.createElement("br"));
      if (chunk) panel.appendChild(document.createTextNode(chunk));
    });
  };
  const appendToken = (role: string, text: string) => {
    const chunks = text.split("\n");
    chunks.forEach((chunk, idx) => {
      if (idx > 0) panel.appendChild(document.createElement("br"));
      let node;
      if (role === "btn") {
        node = document.createElement("span");
        node.className = "btn-ref";
        node.dataset.btnRef = chunk;
      } else if (role === "italic") {
        node = document.createElement("em");
        node.className = "tok-italic";
      } else {
        node = document.createElement("code");
        node.className = role ? `tok-${role}` : "";
      }
      node.textContent = chunk;
      panel.appendChild(node);
    });
  };
  const appendPart = (part: RenderPart) => {
    if (part === null || part === undefined) return;
    if (typeof part === "string" || typeof part === "number") {
      const parsed = parseStyledText(String(part));
      parsed.forEach((segment) => {
        if (typeof segment === "object" && segment.kind === "tok") {
          const role = String(segment.role || "").trim();
          appendToken(role, segment.text);
        } else {
          appendText(String(segment));
        }
      });
      return;
    }
    if (part && typeof part === "object") {
      if (part.kind === "br") {
        panel.appendChild(document.createElement("br"));
        return;
      }
      if (part.kind === "tok") {
        const role = String(part.role || "").trim();
        appendToken(role, part.text);
        return;
      }
    }
    appendText(part);
  };
  list.forEach(appendPart);
}

function bindBtnRefPulse(root: ParentNode | null = document): void {
  if (!root || (root as HTMLElement).dataset?.btnRefPulseBound === "1") return;
  const host = root as HTMLElement;
  if (host.dataset) host.dataset.btnRefPulseBound = "1";
  const clear = () => {
    document
      .querySelectorAll("button.btn-ref-hover")
      .forEach((btn) => btn.classList.remove("btn-ref-hover"));
  };
  host.addEventListener(
    "mouseover",
    (event) => {
      const target = (event?.target as HTMLElement | null)?.closest?.(
        ".btn-ref",
      ) as HTMLElement | null;
      if (!target) return;
      const label = (target.dataset.btnRef || target.textContent || "").trim();
      if (!label) return;
      document.querySelectorAll("button").forEach((btn) => {
        if (btn.textContent?.trim() === label)
          btn.classList.add("btn-ref-hover");
      });
    },
    true,
  );
  host.addEventListener(
    "mouseout",
    (event) => {
      const target = (event?.target as HTMLElement | null)?.closest?.(
        ".btn-ref",
      ) as HTMLElement | null;
      if (!target) return;
      clear();
    },
    true,
  );
  const observer = new MutationObserver(() => clear());
  observer.observe(document.body, { childList: true, subtree: true });
}

function setPartsContent(
  panel: Element | null,
  parts: Parts | RenderParts | null,
) {
  if (!panel) return;
  if (!parts || (Array.isArray(parts) && parts.length === 0)) {
    clearNode(panel);
    panel.classList.add("hidden");
    return;
  }
  panel.classList.remove("hidden");
  renderParts(panel, parts);
}

function stepperButtons(
  root: Element | null,
  dir: string,
): HTMLButtonElement[] {
  const list: HTMLButtonElement[] = [];
  if (!root) return list;
  root.querySelectorAll(`[data-stepper="${dir}"]`).forEach((btn) => {
    if (btn instanceof HTMLButtonElement && !list.includes(btn)) list.push(btn);
  });
  return list;
}

function createStepper({
  root,
  prevButtons = null,
  nextButtons = null,
  lines = [],
  nextPage = null,
  getBoundary,
  setBoundary,
  onBeforeChange,
  onAfterChange,
  isStepLocked,
  getStepBadge,
  getNextLabel,
  getNextBoundary,
  getPrevBoundary,
  isAtEnd,
  endLabel,
  allowSameBoundary = false,
}: StepperOptions = {}): Stepper {
  const boundButtons = new WeakSet();
  const getPrevButtons = () =>
    prevButtons || stepperButtons(root as Element | null, "prev");
  const getNextButtons = () =>
    nextButtons || stepperButtons(root as Element | null, "next");
  const total = Array.isArray(lines)
    ? lines.length
    : Math.max(0, Number(lines) || 0);

  function clearPulse() {
    getNextButtons().forEach((btn) => btn.classList.remove("pulse-success"));
  }

  function boundary() {
    return typeof getBoundary === "function" ? getBoundary() : 0;
  }

  function setBoundaryValue(value: number) {
    if (typeof setBoundary === "function") setBoundary(value);
  }

  function atEnd(at: number) {
    return typeof isAtEnd === "function" ? !!isAtEnd(at, total) : at === total;
  }

  function locked(at: number) {
    return typeof isStepLocked === "function"
      ? !!isStepLocked(at, atEnd(at))
      : false;
  }

  function update() {
    bindButtons();
    const prevButtons = getPrevButtons();
    const nextButtons = getNextButtons();
    const current = boundary();
    prevButtons.forEach((btn) => {
      btn.disabled = current === 0;
    });
    if (nextButtons.length) {
      const atEndNow = atEnd(current);
      const isLocked = locked(current);
      const customLabel =
        typeof getNextLabel === "function"
          ? getNextLabel(current, total, atEndNow)
          : "";
      const badge =
        !atEndNow && typeof getStepBadge === "function"
          ? getStepBadge(current + 1)
          : "";
      const badgeTag = badge === "note" ? "🔧" : badge === "check" ? "✅" : "";
      const labelPrefix = badgeTag ? `${badgeTag} ` : "";
      const adjustLabelForBadge = (label: string) => {
        if (badge !== "note") return label;
        if (label.startsWith("Run line ")) {
          return `Solve line ${label.slice("Run line ".length)}`;
        }
        if (label.startsWith("Run lines ")) {
          return `Solve lines ${label.slice("Run lines ".length)}`;
        }
        return label;
      };
      if (atEndNow) {
        const label = customLabel || endLabel || "Next Program";
        nextButtons.forEach((btn) => {
          const baseText = `${labelPrefix}${label} ▶▶`;
          btn.textContent = isLocked ? `${baseText} 🔒` : baseText;
          btn.dataset.stepperEnd = "true";
        });
      } else {
        const label = customLabel || `Run line ${current + 1}`;
        const adjustedLabel = adjustLabelForBadge(label);
        const isEndPreview =
          !!customLabel && !!endLabel && customLabel === endLabel;
        const arrowSuffix = isEndPreview ? "▶▶" : "▶";
        nextButtons.forEach((btn) => {
          const baseText = `${labelPrefix}${adjustedLabel} ${arrowSuffix}`;
          btn.textContent = isLocked ? `${baseText} 🔒` : baseText;
          btn.dataset.stepperEnd = "false";
        });
      }
      nextButtons.forEach((btn) => {
        btn.disabled = isLocked;
      });
    }
  }

  function sidebarParamValue() {
    return document.body.classList.contains("sidebar-collapsed") ? "0" : "1";
  }

  function withSidebarParam(url: string | null) {
    if (!url) return url;
    const [base, hash = ""] = url.split("#");
    const [path, query = ""] = base.split("?");
    const params = new URLSearchParams(query);
    params.set("sidebar", sidebarParamValue());
    const nextQuery = params.toString();
    const hashPart = hash ? `#${hash}` : "";
    return nextQuery ? `${path}?${nextQuery}${hashPart}` : `${path}${hashPart}`;
  }

  function goTo(target: number) {
    const current = boundary();
    const clamped = Math.max(0, Math.min(total, target));
    if (clamped === current) {
      if (!allowSameBoundary) return;
      onBeforeChange?.(current);
      onAfterChange?.(clamped);
      update();
      return;
    }
    onBeforeChange?.(current);
    setBoundaryValue(clamped);
    onAfterChange?.(clamped);
    update();
  }

  function bindButtons() {
    getPrevButtons().forEach((btn) => {
      if (boundButtons.has(btn)) return;
      boundButtons.add(btn);
      btn.addEventListener("click", () => {
        if (boundary() === 0) return;
        clearPulse();
        const current = boundary();
        const target =
          typeof getPrevBoundary === "function"
            ? getPrevBoundary(current, total)
            : current - 1;
        goTo(target);
      });
    });

    getNextButtons().forEach((btn) => {
      if (boundButtons.has(btn)) return;
      boundButtons.add(btn);
      btn.addEventListener("click", () => {
        const current = boundary();
        clearPulse();
        if (atEnd(current)) {
          if (!btn.disabled && nextPage) {
            const nextUrl = withSidebarParam(nextPage);
            if (nextUrl) window.location.href = nextUrl;
          }
          return;
        }
        if (locked(current)) return;
        const target =
          typeof getNextBoundary === "function"
            ? getNextBoundary(current, total)
            : current + 1;
        goTo(target);
      });
    });
  }

  bindButtons();
  update();

  return {
    update,
    goTo,
    boundary,
    clearPulse,
    pulseNext: () => {
      getNextButtons().forEach((btn) => btn.classList.add("pulse-success"));
    },
  };
}

function syncPlaceholderMutedState(node: HTMLElement | null) {
  if (!node || !node.classList?.contains("placeholder")) return;
  if (txt(node) === "") node.classList.add("muted");
  else node.classList.remove("muted");
}

function initEditableFieldHandlers() {
  if (document.body.dataset.editableFieldHandlersReady === "1") return;
  document.body.dataset.editableFieldHandlersReady = "1";

  document.addEventListener("focusin", (event) => {
    const target = event.target as HTMLElement | null;
    if (!target) return;
    disableAutoText(target);
    if (
      target.classList?.contains("code-editable") &&
      target.classList.contains("placeholder")
    ) {
      const placeholder = target.dataset?.placeholder || "";
      if (txt(target) === placeholder) {
        target.textContent = "";
        target.classList.remove("placeholder", "muted");
      }
    }
    syncPlaceholderMutedState(target);
  });

  document.addEventListener("input", (event) => {
    const target = event.target as HTMLElement | null;
    syncPlaceholderMutedState(target);
  });

  document.addEventListener("keydown", (event) => {
    if (event.key !== "Enter") return;
    const target = event.target as HTMLElement | null;
    if (!target?.isContentEditable) return;
    if (
      target.classList?.contains("value") ||
      target.classList?.contains("type") ||
      target.classList?.contains("name-text")
    ) {
      event.preventDefault();
      target.blur();
    }
  });

  document.addEventListener("focusout", (event) => {
    const target = event.target as HTMLElement | null;
    if (!target) return;
    if (target.classList?.contains("code-editable")) {
      const placeholder = target.dataset?.placeholder || "";
      if (!txt(target)) {
        target.textContent = placeholder;
        if (placeholder) target.classList.add("placeholder", "muted");
      }
    }
    if (target.classList?.contains("placeholder") && txt(target) === "") {
      target.textContent = "";
      target.classList.add("muted");
    }
  });
}

function isTextInputActive(el: HTMLElement | null) {
  if (!el) return false;
  if (el.isContentEditable) return true;
  const tag = el.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT";
}

document.addEventListener("keydown", (e) => {
  if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
  if (
    isTextInputActive(document.activeElement as HTMLElement | null) ||
    isTextInputActive(e.target as HTMLElement | null)
  )
    return;
  const selector =
    e.key === "ArrowLeft"
      ? 'button[data-stepper="prev"]'
      : 'button[data-stepper="next"]';
  const btn = [...document.querySelectorAll(selector)].find(
    (node): node is HTMLButtonElement =>
      node instanceof HTMLButtonElement &&
      !node.disabled &&
      node.dataset.stepperEnd !== "true",
  );
  if (!btn || btn.disabled) return;
  e.preventDefault();
  btn.click();
});

type CustomScrollbarState = {
  host: HTMLElement;
  panel: HTMLElement;
  trackY: HTMLDivElement;
  thumbY: HTMLDivElement;
  trackX: HTMLDivElement;
  thumbX: HTMLDivElement;
  rafId: number | null;
};

const customScrollbarState = new WeakMap<HTMLElement, CustomScrollbarState>();

function initCustomPanelScrollbars() {
  const states = new Set<CustomScrollbarState>();
  const selectors = [
    ".panel-scroll > .panel-body",
    ".state-panel.state-panel-scrollable .state-panel-scroll-body",
    ".sandbox-expr-row [data-role=\"sandbox-expr-result\"]",
    ".expr-answer-result",
  ];

  const updateState = (state: CustomScrollbarState) => {
    const { host, panel, trackY, thumbY, trackX, thumbX } = state;
    if (!host.isConnected || !panel.isConnected) {
      trackY.remove();
      trackX.remove();
      states.delete(state);
      return;
    }
    if (isMobileViewport()) {
      trackY.classList.add("hidden");
      trackX.classList.add("hidden");
      return;
    }
    const hostRect = host.getBoundingClientRect();
    const panelRect = panel.getBoundingClientRect();
    const left = Math.max(0, hostRect.left - panelRect.left);
    const top = Math.max(0, hostRect.top - panelRect.top);
    const scrollableY = Math.max(0, host.scrollHeight - host.clientHeight);
    const scrollableX = Math.max(0, host.scrollWidth - host.clientWidth);
    const hasY = host.clientHeight > 0 && scrollableY > 1;
    const hasX = host.clientWidth > 0 && scrollableX > 1;

    if (!hasY) {
      trackY.classList.add("hidden");
    } else {
      const visibleY = host.clientHeight;
      const trackHeight = Math.max(0, visibleY - (hasX ? 14 : 0));
      trackY.style.top = `${Math.round(top)}px`;
      trackY.style.height = `${Math.round(trackHeight)}px`;
      trackY.classList.remove("hidden");
      const thumbHeight = Math.max(
        28,
        Math.round((visibleY * visibleY) / host.scrollHeight),
      );
      const maxTravel = Math.max(0, trackHeight - thumbHeight);
      const ratio = scrollableY > 0 ? host.scrollTop / scrollableY : 0;
      const thumbTop = Math.max(0, Math.min(maxTravel, Math.round(maxTravel * ratio)));
      thumbY.style.height = `${thumbHeight}px`;
      thumbY.style.transform = `translateY(${thumbTop}px)`;
    }

    if (!hasX) {
      trackX.classList.add("hidden");
    } else {
      const visibleX = host.clientWidth;
      const trackWidth = Math.max(0, visibleX - (hasY ? 14 : 0));
      trackX.style.left = `${Math.round(left)}px`;
      trackX.style.top = `${Math.round(top + host.clientHeight - 12)}px`;
      trackX.style.width = `${Math.round(trackWidth)}px`;
      trackX.classList.remove("hidden");
      const thumbWidth = Math.max(
        28,
        Math.round((visibleX * visibleX) / host.scrollWidth),
      );
      const maxTravel = Math.max(0, trackWidth - thumbWidth);
      const ratio = scrollableX > 0 ? host.scrollLeft / scrollableX : 0;
      const thumbLeft = Math.max(0, Math.min(maxTravel, Math.round(maxTravel * ratio)));
      thumbX.style.width = `${thumbWidth}px`;
      thumbX.style.transform = `translateX(${thumbLeft}px)`;
    }
  };

  const scheduleUpdate = (state: CustomScrollbarState) => {
    if (state.rafId != null) return;
    state.rafId = requestAnimationFrame(() => {
      state.rafId = null;
      updateState(state);
    });
  };

  const bindHost = (host: HTMLElement) => {
    if (customScrollbarState.has(host)) return;
    const panel = host.closest(".panel, .state-panel") as HTMLElement | null;
    if (!panel) return;
    host.classList.add("custom-scroll-host");
    const trackY = document.createElement("div");
    trackY.className = "panel-custom-scrollbar panel-custom-scrollbar-y hidden";
    const thumbY = document.createElement("div");
    thumbY.className = "panel-custom-scrollbar-thumb panel-custom-scrollbar-thumb-y";
    trackY.appendChild(thumbY);
    panel.appendChild(trackY);
    const trackX = document.createElement("div");
    trackX.className = "panel-custom-scrollbar panel-custom-scrollbar-x hidden";
    const thumbX = document.createElement("div");
    thumbX.className = "panel-custom-scrollbar-thumb panel-custom-scrollbar-thumb-x";
    trackX.appendChild(thumbX);
    panel.appendChild(trackX);

    const state: CustomScrollbarState = {
      host,
      panel,
      trackY,
      thumbY,
      trackX,
      thumbX,
      rafId: null,
    };
    customScrollbarState.set(host, state);
    states.add(state);

    host.addEventListener("scroll", () => scheduleUpdate(state), { passive: true });

    let draggingY = false;
    let startY = 0;
    let startScrollTop = 0;
    const onMoveY = (event: MouseEvent) => {
      if (!draggingY) return;
      const scrollable = Math.max(0, host.scrollHeight - host.clientHeight);
      if (scrollable <= 0) return;
      const thumbHeight = thumbY.getBoundingClientRect().height;
      const maxTravel = Math.max(
        1,
        trackY.getBoundingClientRect().height - thumbHeight,
      );
      const deltaY = event.clientY - startY;
      const scrollDelta = (deltaY / maxTravel) * scrollable;
      host.scrollTop = Math.max(
        0,
        Math.min(scrollable, startScrollTop + scrollDelta),
      );
      scheduleUpdate(state);
    };
    const stopDraggingY = () => {
      if (!draggingY) return;
      draggingY = false;
      window.removeEventListener("mousemove", onMoveY);
      window.removeEventListener("mouseup", stopDraggingY);
      document.body.classList.remove("custom-scrollbar-dragging");
    };

    thumbY.addEventListener("mousedown", (event) => {
      event.preventDefault();
      draggingY = true;
      startY = event.clientY;
      startScrollTop = host.scrollTop;
      document.body.classList.add("custom-scrollbar-dragging");
      window.addEventListener("mousemove", onMoveY);
      window.addEventListener("mouseup", stopDraggingY);
    });

    trackY.addEventListener("mousedown", (event) => {
      if (event.target === thumbY) return;
      event.preventDefault();
      const rect = trackY.getBoundingClientRect();
      const offset = event.clientY - rect.top;
      const ratio = Math.max(0, Math.min(1, offset / Math.max(1, rect.height)));
      const scrollable = Math.max(0, host.scrollHeight - host.clientHeight);
      host.scrollTop = Math.round(scrollable * ratio);
      scheduleUpdate(state);
    });

    let draggingX = false;
    let startX = 0;
    let startScrollLeft = 0;
    const onMoveX = (event: MouseEvent) => {
      if (!draggingX) return;
      const scrollable = Math.max(0, host.scrollWidth - host.clientWidth);
      if (scrollable <= 0) return;
      const thumbWidth = thumbX.getBoundingClientRect().width;
      const maxTravel = Math.max(
        1,
        trackX.getBoundingClientRect().width - thumbWidth,
      );
      const deltaX = event.clientX - startX;
      const scrollDelta = (deltaX / maxTravel) * scrollable;
      host.scrollLeft = Math.max(
        0,
        Math.min(scrollable, startScrollLeft + scrollDelta),
      );
      scheduleUpdate(state);
    };
    const stopDraggingX = () => {
      if (!draggingX) return;
      draggingX = false;
      window.removeEventListener("mousemove", onMoveX);
      window.removeEventListener("mouseup", stopDraggingX);
      document.body.classList.remove("custom-scrollbar-dragging");
    };

    thumbX.addEventListener("mousedown", (event) => {
      event.preventDefault();
      draggingX = true;
      startX = event.clientX;
      startScrollLeft = host.scrollLeft;
      document.body.classList.add("custom-scrollbar-dragging");
      window.addEventListener("mousemove", onMoveX);
      window.addEventListener("mouseup", stopDraggingX);
    });

    trackX.addEventListener("mousedown", (event) => {
      if (event.target === thumbX) return;
      event.preventDefault();
      const rect = trackX.getBoundingClientRect();
      const offset = event.clientX - rect.left;
      const ratio = Math.max(0, Math.min(1, offset / Math.max(1, rect.width)));
      const scrollable = Math.max(0, host.scrollWidth - host.clientWidth);
      host.scrollLeft = Math.round(scrollable * ratio);
      scheduleUpdate(state);
    });

    if (typeof ResizeObserver !== "undefined") {
      const ro = new ResizeObserver(() => scheduleUpdate(state));
      ro.observe(host);
      ro.observe(panel);
    }
    scheduleUpdate(state);
  };

  const scan = () => {
    const seen = new Set<HTMLElement>();
    selectors.forEach((selector) => {
      document.querySelectorAll(selector).forEach((node) => {
        if (!(node instanceof HTMLElement) || seen.has(node)) return;
        seen.add(node);
        bindHost(node);
      });
    });
  };

  const updateAll = () => {
    for (const state of [...states]) {
      scheduleUpdate(state);
    }
  };

  scan();
  window.addEventListener("resize", updateAll, { passive: true });
  const rootObserver = new MutationObserver(() => {
    scan();
    updateAll();
  });
  rootObserver.observe(document.body, { childList: true, subtree: true });
}

function applySidebarStateFromUrl() {
  if (document.body.dataset.sidebarStateApplied === "1") return;
  const params = new URLSearchParams(window.location.search);
  const state = params.get("sidebar");
  if (state === "0") document.body.classList.add("sidebar-collapsed");
  if (state === "1") document.body.classList.remove("sidebar-collapsed");
  if (state == null) {
    const prefersCollapsed = isMobileViewport();
    if (prefersCollapsed) document.body.classList.add("sidebar-collapsed");
  }
  document.body.dataset.sidebarStateApplied = "1";
}

onDomReady(() => {
  initEditableFieldHandlers();
  applyAutoTextDefaults(document);
  initCustomPanelScrollbars();
  ensureBaseLayout();
});

function flashStatus(el: Element | null) {
  const node = el as HTMLElement | null;
  if (!node) return;
  node.classList.remove("status-flash");
  // force reflow to restart animation
  void node.offsetWidth;
  node.classList.add("status-flash");
}

export {
  clearNode,
  buildNav,
  createStepper,
  disableBoxEditing,
  ensureBaseLayout,
  ensurePanelizedMain,
  flashStatus,
  getNavLabelForHref,
  isMobileViewport,
  bindBtnRefPulse,
  makeAnswerBox,
  queryElement,
  queryRole,
  readBoxState,
  removeBoxDeleteButtons,
  renderCodePane,
  renderParts,
  resolveActiveNavItem,
  restoreWorkspace,
  serializeWorkspace,
  setPartsContent,
  syncDocumentTitleFromNav,
  vbox,
  applyOtherNames,
  appendStateObjects,
  findArrayObjectBoxesForResult,
};
