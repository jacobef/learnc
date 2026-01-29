import type { BoxState, BoxValue } from "./shared-core-utils.js";
import { DEFAULT_NAV_ITEMS as NAV_ITEMS } from "./nav-items.js";
import {
  doubleDisplayIsExact,
  formatDoubleDefault,
  formatDoubleExact,
  formatDoubleStorage,
  formatValueForType,
  getPointerDepth,
  normalizeZeroDisplay,
  parseDoubleValueWithSign,
  parseType,
  randAddr,
  typeInfo,
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

function $(selector: string, root: ParentNode = document): Element | null {
  return root.querySelector(selector);
}

function el(html: string): HTMLElement {
  const t = document.createElement("template");
  t.innerHTML = html.trim();
  return t.content.firstElementChild as HTMLElement;
}

function txt(n?: Node | null): string {
  return (n?.textContent || "").trim();
}

function disableAutoText(el?: Element | null) {
  if (!el || el.nodeType !== 1) return;
  el.setAttribute("autocapitalize", "off");
  el.setAttribute("autocorrect", "off");
  el.setAttribute("autocomplete", "off");
  el.setAttribute("spellcheck", "false");
}

function applyAutoTextDefaults(root: ParentNode = document) {
  if (!root) return;
  root
    .querySelectorAll(
      'input[type="text"], input:not([type]), textarea, [contenteditable="true"]',
    )
    .forEach((el) => disableAutoText(el));
}

type StepperTopEntry = {
  top: HTMLDivElement;
  update: (() => void) | null;
  scheduled: boolean;
  needsTop: boolean | null;
  measure: (() => void) | null;
  locked: boolean;
  lockOnMeasure: boolean;
  ro?: ResizeObserver;
};

const stepperTopState = new Map<Element, StepperTopEntry>();

function findStepperControls(
  panel: Element | null,
): { prev: HTMLButtonElement; next: HTMLButtonElement } | null {
  if (!panel) return null;
  const controls = [...panel.querySelectorAll(".controls")].find((el) => {
    return (
      el.querySelector('button[data-stepper="prev"]') &&
      el.querySelector('button[data-stepper="next"]')
    );
  });
  if (!controls) return null;
  const prev = controls.querySelector(
    'button[data-stepper="prev"]',
  ) as HTMLButtonElement | null;
  const next = controls.querySelector(
    'button[data-stepper="next"]',
  ) as HTMLButtonElement | null;
  if (!prev || !next) return null;
  return { prev, next };
}

function ensureStepperTopControls(codepane: Element | null): {
  top: HTMLDivElement;
  update: (() => void) | null;
  scheduled: boolean;
  needsTop: boolean | null;
  measure: (() => void) | null;
  locked: boolean;
  lockOnMeasure: boolean;
  ro?: ResizeObserver;
} | null {
  if (!codepane) return null;
  if (stepperTopState.has(codepane))
    return stepperTopState.get(codepane) || null;
  const panel = codepane.closest(".panel");
  if (!panel) return null;
  const info = findStepperControls(panel);
  if (!info) return null;
  const controls = info.prev.closest(".controls") as HTMLElement | null;
  let top = panel.querySelector(".controls-top") as HTMLDivElement | null;
  if (!top) {
    top = document.createElement("div");
    top.className = "controls controls-top";
    const prevBtn = document.createElement("button");
    prevBtn.dataset.stepper = "prev";
    prevBtn.textContent = info.prev.textContent || "Back ◀";
    const nextBtn = document.createElement("button");
    nextBtn.dataset.stepper = "next";
    nextBtn.textContent = info.next.textContent || "Run line 1 ▶";
    top.appendChild(prevBtn);
    top.appendChild(nextBtn);
    panel.insertBefore(top, codepane);
  }
  const entry: StepperTopEntry = {
    top,
    update: null,
    scheduled: false,
    needsTop: null,
    measure: null,
    locked: false,
    lockOnMeasure: true,
  };
  const measure = () => {
    if (entry.locked || !document.body.contains(codepane)) return;
    const rect = codepane.getBoundingClientRect();
    const panelRect = panel.getBoundingClientRect();
    const viewHeight =
      window.innerHeight || document.documentElement.clientHeight || 0;
    const edgeEpsilon = 1;
    const height = Math.max(
      panel.scrollHeight || 0,
      panelRect.height || 0,
      codepane.scrollHeight || 0,
      rect.height || 0,
    );
    if (height === 0 || viewHeight === 0) return;
    const needsBottom =
      height >= viewHeight ||
      panelRect.bottom > viewHeight - edgeEpsilon ||
      rect.bottom > viewHeight - edgeEpsilon;
    entry.needsTop = needsBottom;
    if (controls) controls.classList.toggle("hidden", !needsBottom);
    if (entry.lockOnMeasure) entry.locked = true;
  };
  const update = () => {
    if (entry.locked || entry.scheduled) return;
    entry.scheduled = true;
    requestAnimationFrame(() => {
      entry.scheduled = false;
      measure();
    });
  };
  entry.update = update;
  entry.measure = measure;
  stepperTopState.set(codepane, entry);
  if (typeof ResizeObserver !== "undefined") {
    const ro = new ResizeObserver(() => update());
    ro.observe(codepane);
    entry.ro = ro;
  }
  update();
  return entry;
}

function updateStepperTopControls(codepane: Element | null) {
  const entry = ensureStepperTopControls(codepane);
  entry?.update?.();
}

const MOBILE_MEDIA_QUERY = "(max-width: 900px)";

function isMobileViewport(): boolean {
  return window.matchMedia && window.matchMedia(MOBILE_MEDIA_QUERY).matches;
}

function isStepperTopVisible(codepane: Element | null): boolean {
  if (!codepane) return false;
  const entry = ensureStepperTopControls(codepane);
  if (!entry) return false;
  if (!entry.locked && typeof entry.measure === "function") entry.measure();
  return !!entry.needsTop;
}

const DEFAULT_NAV_ITEMS: NavItem[] = NAV_ITEMS;


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
  const list = Array.isArray(items) && items.length ? items : DEFAULT_NAV_ITEMS;
  const current = normalizeNavHref(activeHref || currentNavHref());
  return list.find((item) => normalizeNavHref(item?.href || "") === current);
}

function buildNav(
  items: NavItem[] = DEFAULT_NAV_ITEMS,
  { activeHref }: { activeHref?: string } = {},
): HTMLElement {
  const list = Array.isArray(items) && items.length ? items : DEFAULT_NAV_ITEMS;
  const current = normalizeNavHref(activeHref || currentNavHref());
  const nav = document.createElement("nav");
  nav.className = "tabs";
  list.forEach((item) => {
    if (!item) return;
    const link = document.createElement("a");
    link.href = item.href || "#";
    link.textContent = item.label || "";
    if (normalizeNavHref(item.href || "") === current) {
      link.classList.add("active");
      link.setAttribute("aria-current", "page");
    }
    nav.appendChild(link);
  });
  return nav;
}

function ensureBaseLayout({
  navItems,
  activeHref,
}: { navItems?: NavItem[]; activeHref?: string } = {}) {
  let wrap = document.querySelector(".wrap") as HTMLElement | null;
  let nav = (wrap?.querySelector("nav.tabs") ||
    document.querySelector("nav.tabs")) as HTMLElement | null;
  let main = (wrap?.querySelector(".main") ||
    document.querySelector(".main")) as HTMLElement | null;
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
  if (!wrap.isConnected) {
    const mount = document.body;
    const firstScript = mount?.querySelector("script");
    const anchor =
      main?.parentElement === mount
        ? main
        : nav?.parentElement === mount
          ? nav
          : null;
    if (mount) {
      if (anchor) mount.insertBefore(wrap, anchor);
      else if (firstScript) mount.insertBefore(wrap, firstScript);
      else mount.appendChild(wrap);
    }
  }
  if (nav.parentElement !== wrap) {
    wrap.appendChild(nav);
  }
  if (main.parentElement !== wrap) {
    wrap.appendChild(main);
  }
  if (nav.nextElementSibling !== main) {
    wrap.insertBefore(nav, main);
  }
  requestAnimationFrame(() => {
    ensureNavSelectionVisible(nav);
    bindNavScrollIndicators(nav);
  });
  return {
    wrap: wrap as HTMLElement,
    nav: nav as HTMLElement,
    main: main as HTMLElement,
  };
}

function bindNavScrollIndicators(nav: HTMLElement | null) {
  if (!nav || nav.dataset.scrollIndicatorsBound === "true") return;
  nav.dataset.scrollIndicatorsBound = "true";
  const update = () => {
    adjustNavHeight(nav);
    updateNavScrollIndicators(nav);
  };
  nav.addEventListener("scroll", update, { passive: true });
  window.addEventListener("resize", update, { passive: true });
  update();
}

function adjustNavHeight(nav: HTMLElement | null) {
  if (!nav) return;
  const links = [...nav.querySelectorAll("a")];
  if (!links.length) return;
  const first = links[0];
  if (!(first instanceof HTMLElement)) return;
  const rect = first.getBoundingClientRect();
  if (!rect.height) return;
  const style = window.getComputedStyle(first);
  const marginTop = parseFloat(style.marginTop) || 0;
  const marginBottom = parseFloat(style.marginBottom) || 0;
  const itemHeight = rect.height + marginTop + marginBottom;
  const maxHeight = window.innerHeight * 0.8;
  const halfItem = itemHeight * 0.5;
  const count = Math.max(0, Math.floor((maxHeight - halfItem) / itemHeight));
  const roundedHeight = Math.max(
    halfItem,
    Math.min(maxHeight, count * itemHeight + halfItem),
  );
  const fullHeight = Math.min(maxHeight, nav.scrollHeight);
  const activeIndex = links.findIndex((link) =>
    link.classList.contains("active"),
  );
  const nearBottom = activeIndex >= links.length - 2;
  const remainder = nav.scrollHeight - roundedHeight;
  const targetHeight =
    nearBottom || remainder < itemHeight ? fullHeight : roundedHeight;
  nav.style.height = `${Math.round(targetHeight)}px`;
}

function updateNavScrollIndicators(nav: HTMLElement | null) {
  if (!nav) return;
  const maxScroll = Math.max(0, nav.scrollHeight - nav.clientHeight);
  const top = Math.max(0, nav.scrollTop);
  nav.classList.toggle("nav-scroll-top", top > 4);
  nav.classList.toggle("nav-scroll-bottom", top < maxScroll - 4);
}

function ensureNavSelectionVisible(nav: HTMLElement | null) {
  if (!nav) return;
  const links = [...nav.querySelectorAll("a")];
  if (!links.length) return;
  const activeIndex = links.findIndex((link) =>
    link.classList.contains("active"),
  );
  if (activeIndex < 0) return;
  const active = links[activeIndex];
  const next = links[activeIndex + 1] || active;
  const nextNext = links[activeIndex + 2] || next;
  const top = Math.min(active.offsetTop, next.offsetTop, nextNext.offsetTop);
  const bottom = Math.max(
    active.offsetTop + active.offsetHeight,
    next.offsetTop + next.offsetHeight,
    nextNext.offsetTop + nextNext.offsetHeight,
  );
  const padding = 8;
  const viewTop = nav.scrollTop;
  const viewBottom = viewTop + nav.clientHeight;
  if (top < viewTop + padding) {
    nav.scrollTop = Math.max(0, top - padding);
    return;
  }
  if (bottom > viewBottom - padding) {
    nav.scrollTop = Math.max(0, bottom - nav.clientHeight + padding);
  }
}

function initStepperTopControls() {
  document.querySelectorAll(".codepane").forEach((pane) => {
    updateStepperTopControls(pane);
  });
}

function renderCodePane(
  root: Element,
  lines: string[],
  boundary: number,
  opts: RenderCodePaneOptions = {},
) {
  root.innerHTML = "";
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
          typeof opts.selectedBoundary === "number" &&
          Number.isFinite(opts.selectedBoundary) &&
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
  let doneBoundary = boundary;
  if (
    typeof opts.doneBoundary === "number" &&
    Number.isFinite(opts.doneBoundary)
  ) {
    doneBoundary = Math.max(0, Math.min(lines.length, opts.doneBoundary));
  }
  if (progress) {
    const range = opts.progressRange;
    let rangeStart: number | null = null;
    let rangeEnd: number | null = null;
    if (Array.isArray(range) && range.length >= 2) {
      rangeStart = Number(range[0]);
      rangeEnd = Number(range[1]);
    } else if (range && typeof range === "object" && !Array.isArray(range)) {
      rangeStart = Number(range.start);
      rangeEnd = Number(range.end);
    }
    if (
      typeof rangeStart === "number" &&
      typeof rangeEnd === "number" &&
      Number.isFinite(rangeStart) &&
      Number.isFinite(rangeEnd)
    ) {
      const start = Math.min(rangeStart, rangeEnd);
      const end = Math.max(rangeStart, rangeEnd);
      const maxIndex = Math.max(0, lines.length - 1);
      progressRangeStart = Math.max(0, Math.min(maxIndex, start));
      progressRangeEnd = Math.max(0, Math.min(maxIndex, end));
      if (
        typeof opts.progressIndex === "number" &&
        Number.isFinite(opts.progressIndex)
      ) {
        progressIndex = Math.max(
          0,
          Math.min(lines.length - 1, opts.progressIndex),
        );
      } else if (!opts.suppressProgressMid && progressRangeStart != null) {
        progressIndex = progressRangeStart;
      }
    } else if (
      typeof opts.progressIndex === "number" &&
      Number.isFinite(opts.progressIndex)
    ) {
      progressIndex = Math.max(
        0,
        Math.min(lines.length - 1, opts.progressIndex),
      );
    } else if (!opts.suppressProgressMid && boundary > 0) {
      progressIndex = boundary - 1;
    }
  }
  const appendStrike = (range: [number, number] | { start: number; end: number }) => {
    let start = Number(Array.isArray(range) ? range[0] : range.start);
    let end = Number(Array.isArray(range) ? range[1] : range.end);
    if (!Number.isFinite(start) || !Number.isFinite(end)) return;
    const maxIndex = Math.max(0, lines.length - 1);
    start = Math.max(0, Math.min(maxIndex, Math.min(start, end)));
    end = Math.max(0, Math.min(maxIndex, Math.max(start, end)));
    strikeRanges.push([start, end]);
  };
  if (opts.strikeRange) appendStrike(opts.strikeRange);
  if (Array.isArray(opts.strikeRanges)) {
    opts.strikeRanges.forEach((range) => appendStrike(range));
  }
  if (doneBoundary === 0 && !hideBoundary) addBoundary();
  if (selectableBoundaries && selectableBoundaries.has(0)) {
    addBoundary(0, true);
  }
  for (let i = 0; i < lines.length; i++) {
    const lr = el('<div class="line"></div>');
    const ln = el(`<div class="ln">${i + 1}</div>`);
    const src = el('<div class="src"></div>');
    src.textContent = lines[i];
    if (i < doneBoundary) lr.classList.add("done");
    const inProgressRange =
      progressRangeStart !== null &&
      progressRangeEnd !== null &&
      i >= progressRangeStart &&
      i <= progressRangeEnd;
    if (inProgressRange) lr.classList.add("progress-range");
    const inStrikeRange = strikeRanges.some(
      ([start, end]) => i >= start && i <= end,
    );
    if (inStrikeRange) lr.classList.add("skipped");
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
  updateStepperTopControls(root);
}

let nameStackResizeInstalled = false;
function updateNameStackSpacing(node: HTMLElement | null): void {
  if (!node) return;
  const stack = node.querySelector(".name-stack");
  if (!stack) return;
  if (!node.isConnected) return;
  const boxRect = node.getBoundingClientRect();
  const stackRect = stack.getBoundingClientRect();
  const overflow = Math.max(0, Math.ceil(stackRect.bottom - boxRect.bottom));
  node.style.setProperty("--name-stack-space", `${overflow}px`);
}

function watchNameStack(node: HTMLElement | null): void {
  const stack = node?.querySelector(".name-stack");
  if (!stack) return;
  const update = () => updateNameStackSpacing(node);
  requestAnimationFrame(update);
  if (typeof ResizeObserver !== "undefined") {
    const ro = new ResizeObserver(update);
    ro.observe(stack);
  } else if (!nameStackResizeInstalled) {
    nameStackResizeInstalled = true;
    window.addEventListener("resize", () => {
      document
        .querySelectorAll<HTMLElement>(".vbox")
        .forEach((box) => updateNameStackSpacing(box));
    });
  }
}

function adjustValueOverflow(node: HTMLElement | null): void {
  if (!node) return;
  if (!node.isConnected) return;
  const valueEl = node.querySelector(".value") as HTMLElement | null;
  const cell = node.querySelector(".cell") as HTMLElement | null;
  if (!valueEl || !cell) return;
  let baseHeight = Number(node.dataset.baseHeight);
  if (!baseHeight) {
    const computed = parseFloat(getComputedStyle(node).height || "");
    baseHeight =
      Number.isFinite(computed) && computed > 0
        ? computed
        : node.getBoundingClientRect().height;
    if (!baseHeight) baseHeight = 200;
    node.dataset.baseHeight = String(baseHeight);
  }
  let baseNameStackTop = Number(node.dataset.baseNameStackTop);
  if (!baseNameStackTop) {
    const rawTop = getComputedStyle(node).getPropertyValue("--name-stack-top");
    const parsedTop = parseFloat(rawTop || "");
    baseNameStackTop =
      Number.isFinite(parsedTop) && parsedTop > 0 ? parsedTop : 160;
    node.dataset.baseNameStackTop = String(baseNameStackTop);
  }
  const valueHeight = Math.ceil(valueEl.scrollHeight);
  const measuredCellHeight = Math.ceil(cell.getBoundingClientRect().height);
  const expectedCellHeight = Math.ceil(baseHeight - 92);
  const extra = Math.max(0, valueHeight - expectedCellHeight + 56);
  const nextHeight = Math.ceil(baseHeight + extra);
  const nextNameTop = Math.ceil(baseNameStackTop + extra);
  const currentHeight = parseFloat(node.style.height || "") || baseHeight;
  if (Math.abs(currentHeight - nextHeight) >= 1) {
    node.style.height = `${nextHeight}px`;
  }
  node.style.setProperty("--name-stack-top", `${nextNameTop}px`);
  updateNameStackSpacing(node);
  const nextCellHeight = Math.ceil(cell.getBoundingClientRect().height);
  if (Math.abs(nextCellHeight - measuredCellHeight) >= 1) {
    requestAnimationFrame(() => adjustValueOverflow(node));
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
  const isDoubleScalar = parsedType.base === "double" && parsedType.depth === 0;
  const rawValue = String(value ?? "");
  const emptyDisplay = rawValue === "";
  const displayValue = emptyDisplay ? "" : normalizeZeroDisplay(rawValue);
  const resolvedName = name !== undefined && name !== null ? String(name) : "";
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
      <div class="lbl lbl-addr">address</div>
      <div class="address">${address}</div>
        <div class="cell">
        <div class="lbl lbl-value">value</div>
        <div class="${valueClasses}">${displayValue}</div>
        <button class="double-toggle hidden" type="button" aria-pressed="false">exact</button>
      </div>
      <div class="lbl lbl-type">type</div>
      <div class="${typeClasses}">${type}</div>
      <div class="name-stack">
        <div class="${listClasses}">
          <div class="name-list-inner">${namesHtml}</div>
        </div>
        <div class="lbl lbl-name">${namesList.length > 1 ? "name(s)" : "name"}</div>
      </div>
    </div>
  `);

  const valueEl = node.querySelector(".value") as HTMLElement | null;
  const scheduleAdjust = () => {
    if ((node.dataset.adjustPending || "") === "true") return;
    node.dataset.adjustPending = "true";
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        node.dataset.adjustPending = "false";
        if (!node.isConnected) return;
        adjustValueOverflow(node);
      });
    });
  };

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
      scheduleAdjust();
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
  if (valueEl && isDoubleScalar && !emptyDisplay && !editable) {
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
          scheduleAdjust();
        });
      }
      if (valueEl.isContentEditable) {
        valueEl.addEventListener("input", () => {
          const next = parseDoubleValueWithSign(valueEl.textContent || "");
          if (next == null) return;
          const nextNanSign = next.nanSign;
          valueEl.dataset.doubleValue = formatDoubleStorage(
            next.value,
            nextNanSign,
          );
          const nextExactText = formatDoubleExact(next.value, nextNanSign);
          const nextDefaultText = formatDoubleDefault(next.value, nextNanSign);
          const nextApprox = !doubleDisplayIsExact(
            nextDefaultText,
            nextExactText,
          );
          const isExact = node.dataset.doubleDisplay === "exact";
          if (nextApprox && !isExact) valueEl.dataset.doubleApprox = "true";
          else delete valueEl.dataset.doubleApprox;
          if (toggleEl) {
            if (nextApprox) {
              toggleEl.classList.remove("hidden");
            } else {
              toggleEl.classList.add("hidden");
            }
          }
          scheduleAdjust();
        });
      }
    }
  } else if (toggleEl) {
    toggleEl.classList.add("hidden");
  }
  watchNameStack(node);
  requestAnimationFrame(scheduleAdjust);
  return node;
}

function disableBoxEditing(root: Element | null) {
  if (!root) return;
  root
    .querySelectorAll(".value.editable, .type.editable, .name-text.editable")
    .forEach((el) => {
      el.removeAttribute("contenteditable");
      el.classList.remove("editable");
    });
  root.classList.remove("is-editable");
}

function removeBoxDeleteButtons(root?: Element | null) {
  const scope = root || document;
  scope.querySelectorAll(".vbox .delete").forEach((btn) => btn.remove());
}

function readBoxState(root: Element | null): BoxState | null {
  if (!root) return null;
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
    parsedType.base === "double" &&
    parsedType.depth === 0 &&
    valEl instanceof HTMLElement &&
    !valueEditable
  ) {
    const stored = valEl.dataset.doubleValue;
    if (stored != null && stored !== "") value = stored;
  }
  const allowDelete =
    el?.dataset?.allowDelete === "true" || !!root.querySelector(".delete");
  return {
    address: txt(root.querySelector(".address")),
    type: typeText,
    value,
    rawValue,
    name: names[0] || "",
    names,
    nameEditable: !!root.querySelector(".name-text[contenteditable]"),
    typeEditable: !!root.querySelector(".type[contenteditable]"),
    allowDelete,
    showDoubleExact: el.dataset.doubleDisplay === "exact",
  };
}

function boxAddress(box: BoxState | null | undefined): string {
  const raw = box?.address ?? "";
  return String(raw ?? "").trim();
}

function collectStageBoxes(root: Element | null): BoxState[] {
  if (!root) return [];
  return [...root.querySelectorAll(".vbox")]
    .map((node) => {
      const box = readBoxState(node);
      if (!box) return null;
      box.node = node as HTMLElement;
      return box;
    })
    .filter((box): box is BoxState => !!box);
}

function pointerTargetBox(
  box: BoxState | null | undefined,
  byAddr: Map<string, BoxState>,
): BoxState | null {
  if (!box) return null;
  const depth = getPointerDepth(box.type);
  if (!Number.isFinite(depth) || depth < 1) return null;
  const raw = String(box.value ?? "").trim();
  if (raw === "") return null;
  const target = byAddr.get(raw) || null;
  if (!target) return null;
  const targetDepth = getPointerDepth(target.type);
  if (!Number.isFinite(targetDepth)) return null;
  if (targetDepth !== depth - 1) return null;
  return target;
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
    const depth = getPointerDepth(box.type);
    if (!baseName || !Number.isFinite(depth) || depth < 1) return;
    let current = box;
    for (let step = 1; step <= depth; step++) {
      const target = pointerTargetBox(current, byAddr);
      if (!target) break;
      const targetAddr = boxAddress(target);
      if (targetAddr) {
        let bucket = otherNamesByAddr.get(targetAddr);
        if (!bucket) {
          bucket = new Set<string>();
          otherNamesByAddr.set(targetAddr, bucket);
        }
        bucket.add(`${"*".repeat(step)}${baseName}`);
      }
      current = target;
      if (getPointerDepth(current.type) < 1) break;
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
  node: HTMLElement | null,
  aliases: string[] | null | undefined,
  showAliases: boolean,
): void {
  if (!node) return;
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
    showAliases && Array.isArray(aliases) && aliases.length > 0;
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
  node: HTMLElement | null,
  onToggle?: ((target: HTMLElement) => void) | null,
): HTMLButtonElement | null {
  if (!node) return null;
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
      if (node) onToggle?.(node);
    });
  }
  return btn;
}

function placeOtherNamesToggle(
  node: HTMLElement | null,
  btn: HTMLButtonElement | null,
  showAliases: boolean,
): void {
  if (!node || !btn) return;
  const listInner = node.querySelector(".name-list-inner");
  const baseTag = listInner?.querySelector(".name-tag");
  const label = node.querySelector(".lbl-name");
  const stack = node.querySelector(".name-stack");
  if (!listInner || !baseTag || !label || !stack) return;
  if (showAliases) {
    btn.classList.add("stacked");
    if (btn.parentElement !== stack || btn.nextElementSibling !== label) {
      stack.insertBefore(btn, label);
    }
    return;
  }
  btn.classList.remove("stacked");
  if (btn.parentElement !== baseTag) {
    baseTag.appendChild(btn);
  }
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
  const useShownSet = shownAddrs && typeof shownAddrs.has === "function";
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
    if (toggle) placeOtherNamesToggle(node, toggle, showAliases);
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
  names = null,
  type = "",
  value = "",
  address = null,
  editable = true,
  deletable = editable,
  allowNameEdit = null,
  allowTypeEdit = null,
  nameEditable = null,
  typeEditable = null,
  showDoubleExact = null,
}: {
  name?: string;
  names?: string[] | string | null;
  type?: string;
  value?: BoxValue;
  address?: string | number | null;
  editable?: boolean;
  deletable?: boolean;
  allowNameEdit?: boolean | null;
  allowTypeEdit?: boolean | null;
  nameEditable?: boolean | null;
  typeEditable?: boolean | null;
  showDoubleExact?: boolean | null;
} = {}) {
  const resolvedAddr =
    address == null ? String(nextPooledAddr(type || "int")) : String(address);
  const resolvedNameEdit =
    allowNameEdit !== null && allowNameEdit !== undefined
      ? allowNameEdit
      : nameEditable !== null && nameEditable !== undefined
        ? nameEditable
        : !name && !(Array.isArray(names) && names.length);
  const resolvedTypeEdit =
    allowTypeEdit !== null && allowTypeEdit !== undefined
      ? allowTypeEdit
      : typeEditable !== null && typeEditable !== undefined
        ? typeEditable
        : !type;
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
  node.dataset.allowNameEdit = resolvedNameEdit ? "true" : "false";
  node.dataset.allowTypeEdit = resolvedTypeEdit ? "true" : "false";
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
  let boxes = [...ws.querySelectorAll(".vbox")];
  if (!boxes.length && wsEl.dataset.inline === "true") {
    const key = wsEl.dataset.workspaceKey || "";
    if (key) {
      boxes = [...document.querySelectorAll(`.vbox[data-workspace="${key}"]`)];
    }
  }
  return boxes.map((v) => readBoxState(v)).filter(Boolean) as BoxState[];
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
  if (Array.isArray(state) && state.length) {
    state.forEach((st) => {
      const allowDelete =
        st.allowDelete !== null && st.allowDelete !== undefined
          ? !!st.allowDelete
          : deletable;
      const node = makeAnswerBox({
        name: st.name,
        type: st.type,
        value: st.rawValue ?? st.value,
        address: st.address ?? null,
        editable,
        deletable: allowDelete,
        allowNameEdit: allowNameEdit ?? st.nameEditable ?? st.allowNameEdit,
        allowTypeEdit: allowTypeEdit ?? st.typeEditable ?? st.allowTypeEdit,
        showDoubleExact: st.showDoubleExact ?? null,
      });
      if (allowDelete) node.dataset.allowDelete = "true";
      if (String(st.value ?? "") === "")
        node.querySelector(".value")?.classList.add("placeholder", "muted");
      wrap.appendChild(node);
    });
  } else if (Array.isArray(defaults)) {
    defaults.forEach((d) => {
      const allowDelete =
        d.allowDelete !== null && d.allowDelete !== undefined
          ? !!d.allowDelete
          : deletable;
      const node = makeAnswerBox({
        name: d.name,
        type: d.type,
        value: d.rawValue ?? d.value,
        address: d.address ?? null,
        editable,
        deletable: allowDelete,
        allowNameEdit: allowNameEdit ?? d.nameEditable ?? d.allowNameEdit,
        allowTypeEdit: allowTypeEdit ?? d.typeEditable ?? d.allowTypeEdit,
        showDoubleExact: d.showDoubleExact ?? null,
      });
      if (allowDelete) node.dataset.allowDelete = "true";
      if (String(d.value ?? "") === "")
        node.querySelector(".value")?.classList.add("placeholder", "muted");
      wrap.appendChild(node);
    });
  }
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
  const raw = String(text ?? "");
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
  panel.innerHTML = "";
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
    const chunks = String(text ?? "").split("\n");
    chunks.forEach((chunk, idx) => {
      if (idx > 0) panel.appendChild(document.createElement("br"));
      let node;
      if (role === "btn") {
        node = document.createElement("span");
        node.className = "btn-ref";
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

function setPartsContent(
  panel: Element | null,
  parts: Parts | RenderParts | null,
) {
  if (!panel) return;
  if (!parts || (Array.isArray(parts) && parts.length === 0)) {
    panel.textContent = "";
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

  function locked(at: number) {
    return typeof isStepLocked === "function"
      ? !!isStepLocked(at, at === total)
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
      const atEnd = current === total;
      const badge =
        !atEnd && typeof getStepBadge === "function"
          ? getStepBadge(current + 1)
          : "";
      const badgeTag = badge === "note" ? "🔧" : badge === "check" ? "✅" : "";
      const isLocked = locked(current);
      const lockTag = isLocked ? " 🔒" : "";
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
      const customLabel =
        typeof getNextLabel === "function"
          ? getNextLabel(current, total, atEnd)
          : "";
      if (atEnd) {
        const label = customLabel || endLabel || "Next Program";
        nextButtons.forEach((btn) => {
          btn.textContent = `${labelPrefix}${label}${lockTag} ▶▶`;
          btn.dataset.stepperEnd = "true";
        });
      } else {
        const label = customLabel || `Run line ${current + 1}`;
        const adjustedLabel = adjustLabelForBadge(label);
        nextButtons.forEach((btn) => {
          btn.textContent = `${labelPrefix}${adjustedLabel}${lockTag} ▶`;
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
        goTo(Number.isFinite(target) ? target : current - 1);
      });
    });

    getNextButtons().forEach((btn) => {
      if (boundButtons.has(btn)) return;
      boundButtons.add(btn);
      btn.addEventListener("click", () => {
        const current = boundary();
        clearPulse();
        if (current === total) {
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
        goTo(Number.isFinite(target) ? target : current + 1);
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

document.addEventListener("focusin", (e) => {
  const t = e.target as HTMLElement | null;
  if (!t) return;
  disableAutoText(t);
  if (
    t.classList?.contains("code-editable") &&
    t.classList.contains("placeholder")
  ) {
    const placeholder = t.dataset?.placeholder || "";
    if (txt(t) === placeholder) {
      t.textContent = "";
      t.classList.remove("placeholder", "muted");
    }
  }
  if (t.classList?.contains("placeholder")) {
    if (txt(t) === "") {
      t.classList.add("muted");
    } else {
      t.classList.remove("muted");
    }
  }
});

document.addEventListener("input", (e) => {
  const t = e.target as HTMLElement | null;
  if (!t) return;
  if (t.classList?.contains("placeholder")) {
    if (txt(t) === "") t.classList.add("muted");
    else t.classList.remove("muted");
  }
});

document.addEventListener("keydown", (e) => {
  if (e.key !== "Enter") return;
  const t = e.target as HTMLElement | null;
  if (!t?.isContentEditable) return;
  if (
    t.classList?.contains("value") ||
    t.classList?.contains("type") ||
    t.classList?.contains("name-text")
  ) {
    e.preventDefault();
    t.blur();
  }
});

function isTextInputActive(el: HTMLElement | null) {
  if (!el) return false;
  if (el.isContentEditable) return true;
  const tag = el.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT";
}

document.addEventListener("keydown", (e) => {
  if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
  if (e.repeat) return;
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

document.addEventListener("focusout", (e) => {
  const t = e.target as HTMLElement | null;
  if (!t) return;
  if (t.classList?.contains("code-editable")) {
    const placeholder = t.dataset?.placeholder || "";
    if (!txt(t)) {
      t.textContent = placeholder;
      if (placeholder) t.classList.add("placeholder", "muted");
    }
  }
  if (t.classList?.contains("placeholder") && txt(t) === "") {
    t.textContent = "";
    t.classList.add("muted");
  }
});

function initScrollHint() {
  if (document.body?.classList?.contains("no-scroll-hint")) return;
  const btn = el(
    '<button class="scroll-down-btn hidden" aria-label="Scroll to bottom">↓</button>',
  );
  document.body.appendChild(btn);

  const shouldShow = () => {
    const doc = document.documentElement;
    const scrollable = doc.scrollHeight - window.innerHeight;
    const nearBottom = window.scrollY > scrollable - 140;
    return scrollable > 200 && !nearBottom;
  };

  const update = () => {
    btn.classList.toggle("hidden", !shouldShow());
  };

  window.addEventListener("scroll", update, { passive: true });
  window.addEventListener("resize", update);
  const observer = new MutationObserver(update);
  observer.observe(document.body, { childList: true, subtree: true });
  btn.addEventListener("click", () => {
    window.scrollTo({
      top: document.documentElement.scrollHeight,
      behavior: "smooth",
    });
  });
  update();
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", initScrollHint);
} else {
  initScrollHint();
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", () => {
    applyAutoTextDefaults(document);
  });
} else {
  applyAutoTextDefaults(document);
}

initStepperTopControls();
if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", initStepperTopControls, {
    once: true,
  });
}

function initInstructionWatcher() {
  return;
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", initInstructionWatcher, {
    once: true,
  });
} else {
  initInstructionWatcher();
}

function applySidebarStateFromUrl() {
  const params = new URLSearchParams(window.location.search);
  const state = params.get("sidebar");
  if (state === "0") document.body.classList.add("sidebar-collapsed");
  if (state === "1") document.body.classList.remove("sidebar-collapsed");
  if (state == null) {
    const prefersCollapsed = isMobileViewport();
    if (prefersCollapsed) document.body.classList.add("sidebar-collapsed");
  }
}

applySidebarStateFromUrl();

function initSidebarToggle() {
  if (document.body.dataset.sidebarToggleReady === "1") return true;
  const wrap = document.querySelector(".wrap");
  const nav = wrap?.querySelector("nav");
  if (!wrap || !nav) return false;
  if (!nav.id) nav.id = "sidebar";
  let sidebarWrap = wrap.querySelector(".sidebar-wrap");
  if (!sidebarWrap) {
    sidebarWrap = document.createElement("div");
    sidebarWrap.className = "sidebar-wrap";
    wrap.insertBefore(sidebarWrap, nav);
    sidebarWrap.appendChild(nav);
  }
  let btn = document.querySelector(".sidebar-toggle");
  if (!btn) {
    btn = el(
      '<button type="button" class="sidebar-toggle"><span class="hamburger" aria-hidden="true"><span></span><span></span><span></span></span><span class="sr-only">Toggle sidebar</span></button>',
    );
    document.body.appendChild(btn);
  }
  btn.setAttribute("aria-controls", nav.id);
  const placeToggle = () => {
    if (btn.parentElement !== sidebarWrap) {
      sidebarWrap.insertBefore(btn, sidebarWrap.firstChild);
    }
  };
  const updateLabel = () => {
    const hidden = document.body.classList.contains("sidebar-collapsed");
    const label = hidden ? "Show sidebar" : "Hide sidebar";
    btn.classList.toggle("is-expanded", !hidden);
    btn.setAttribute("aria-label", label);
    btn.setAttribute("aria-expanded", hidden ? "false" : "true");
    const sr = btn.querySelector(".sr-only");
    if (sr) sr.textContent = label;
  };
  const updateUrl = () => {
    const hidden = document.body.classList.contains("sidebar-collapsed");
    const params = new URLSearchParams(window.location.search);
    params.set("sidebar", hidden ? "0" : "1");
    const query = params.toString();
    const next = `${window.location.pathname}?${query}${window.location.hash}`;
    window.history.replaceState(null, "", next);
  };
  btn.addEventListener("click", () => {
    document.body.classList.toggle("sidebar-collapsed");
    updateLabel();
    placeToggle();
    updateUrl();
  });
  updateLabel();
  placeToggle();
  document.body.dataset.sidebarToggleReady = "1";
  return true;
}

if (document.readyState === "loading") {
  document.addEventListener(
    "DOMContentLoaded",
    () => {
      if (initSidebarToggle()) return;
      const observer = new MutationObserver(() => {
        if (initSidebarToggle()) observer.disconnect();
      });
      observer.observe(document.body, { childList: true, subtree: true });
    },
    { once: true },
  );
} else {
  if (!initSidebarToggle()) {
    const observer = new MutationObserver(() => {
      if (initSidebarToggle()) observer.disconnect();
    });
    observer.observe(document.body, { childList: true, subtree: true });
  }
}

function flashStatus(el: Element | null) {
  const node = el as HTMLElement | null;
  if (!node) return;
  node.classList.remove("status-flash");
  // force reflow to restart animation
  void node.offsetWidth;
  node.classList.add("status-flash");
}

export {
  $,
  buildNav,
  createStepper,
  disableBoxEditing,
  ensureBaseLayout,
  flashStatus,
  getNavLabelForHref,
  isMobileViewport,
  isStepperTopVisible,
  makeAnswerBox,
  readBoxState,
  removeBoxDeleteButtons,
  renderCodePane,
  renderParts,
  resolveActiveNavItem,
  restoreWorkspace,
  serializeWorkspace,
  setPartsContent,
  updateStepperTopControls,
  vbox,
  applyOtherNames,
};
