import { DEFAULT_NAV_ITEMS as NAV_ITEMS } from "./nav-items.js";
import { startLevelReplay } from "./shared-replay.js";
import {
  clearNode,
  disableAutoText,
  el,
  queryElement,
  queryRole,
  txt,
} from "./shared-dom-utils.js";
import {
  applyOtherNames,
  appendStateObjects,
  disableBoxEditing,
  findArrayObjectBoxesForResult,
  makeAnswerBox,
  readBoxState,
  removeBoxDeleteButtons,
  restoreWorkspace,
  serializeWorkspace,
  vbox,
} from "./shared-workspace-dom.js";

export interface NavItem {
  href: string;
  label: string;
}
interface RenderCodePaneOptions {
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
interface TokenPart {
  kind: "tok";
  role: string;
  text: string;
}
export type Part = string;
export type Parts = string | string[];

interface StepperOptions {
  root?: ParentNode | null;
  prevButtons?: HTMLButtonElement[] | null;
  nextButtons?: HTMLButtonElement[] | null;
  lines?: number | string[];
  previousPage?: string | null;
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
  startLabel?: string;
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

function getPreviousNavHref(href?: string | null): string | null {
  const current = normalizeNavHref(href || currentNavHref());
  const index = DEFAULT_NAV_ITEMS.findIndex(
    (item) => normalizeNavHref(item?.href || "") === current,
  );
  return index > 1 ? DEFAULT_NAV_ITEMS[index - 1]?.href ?? null : null;
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
  startLevelReplay(main);
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

function withSidebarParam(url: string | null): string | null {
  if (!url) return url;
  const [base, hash = ""] = url.split("#");
  const [path, query = ""] = base.split("?");
  const params = new URLSearchParams(query);
  params.set(
    "sidebar",
    document.body.classList.contains("sidebar-collapsed") ? "0" : "1",
  );
  const nextQuery = params.toString();
  return `${path}${nextQuery ? `?${nextQuery}` : ""}${hash ? `#${hash}` : ""}`;
}

function createStepper({
  root,
  prevButtons = null,
  nextButtons = null,
  lines = [],
  previousPage = getPreviousNavHref(),
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
  startLabel,
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
  const resolvedStartLabel = (() => {
    if (startLabel) return startLabel;
    const label = getNavLabelForHref(previousPage);
    return label ? `Prev: ${label}` : "Previous Program";
  })();

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
      const atStart = current === 0;
      const canNavigateBack = atStart && !!previousPage;
      btn.disabled = atStart && !canNavigateBack;
      btn.textContent = canNavigateBack
        ? `${resolvedStartLabel} ◀◀`
        : atStart
          ? "At start"
          : "Back ◀";
      btn.dataset.stepperStart = String(atStart);
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
        if (boundary() === 0) {
          if (!btn.disabled && previousPage) {
            const previousUrl = withSidebarParam(previousPage);
            if (previousUrl) window.location.href = previousUrl;
          }
          return;
        }
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
      node.dataset.stepperStart !== "true" &&
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
  withSidebarParam,
  disableBoxEditing,
  ensureBaseLayout,
  ensurePanelizedMain,
  flashStatus,
  getNavLabelForHref,
  getPreviousNavHref,
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
