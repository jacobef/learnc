import { DEFAULT_NAV_ITEMS as NAV_ITEMS } from "./nav-items.js";
import { normalizeZeroDisplay } from "./shared-core-utils.js";
let nextTypeHelpId = 0;
function appendTypeHelpConnector(parent, text, punctuation = false) {
    const connector = document.createElement("span");
    connector.className = punctuation
        ? "type-help-connector type-help-punctuation"
        : "type-help-connector";
    connector.textContent = punctuation ? text : ` ${text} `;
    parent.append(connector);
}
function pluralizeTypeHelpLabel(node) {
    if (node.kind === "pointer") {
        return node.label.replace(/pointer$/, "pointers");
    }
    if (node.kind === "array") {
        return node.label.replace(/^array\b/, "arrays");
    }
    if (node.kind === "function") {
        return node.label.replace(/\bfunction\b/, "functions");
    }
    if (/^(struct|union|enum)\b/.test(node.label) || node.label === "_Bool") {
        return `${node.label} values`;
    }
    return node.label.replace(/([A-Za-z_][A-Za-z0-9_]*)$/, "$1s");
}
function typeHelpIndefiniteArticle(node) {
    if (node.kind === "type" && node.label === "void")
        return null;
    if (/^union\b/i.test(node.label) || node.label === "_Bool")
        return "a";
    const spokenLabel = node.label
        .replace(/^_Atomic\b/i, "atomic")
        .replace(/^_+/, "");
    return /^[aeiou]/i.test(spokenLabel) ? "an" : "a";
}
function connectorWithArticle(prefix, node) {
    const article = typeHelpIndefiniteArticle(node);
    return article ? `${prefix} ${article}`.trim() : prefix;
}
function nextTypeHelpShadeClass(node, kindCounts) {
    if (node.kind !== "pointer" && node.kind !== "array" && node.kind !== "function") {
        return "";
    }
    const occurrence = kindCounts.get(node.kind) ?? 0;
    kindCounts.set(node.kind, occurrence + 1);
    return `type-help-shade-${occurrence % 3}`;
}
function typeHelpTreeRelation(parent, relation) {
    if ((parent.kind === "pointer" && relation === "to")
        || (parent.kind === "array" && relation === "of")) {
        return "";
    }
    const parameter = /^parameter\s+(\d+)$/.exec(relation);
    return parameter ? `arg ${parameter[1]}` : relation;
}
function typeHelpTreeConnector(relation) {
    const connector = document.createElement("div");
    connector.className = "type-help-tree-connector";
    if (relation) {
        connector.classList.add("has-relation");
        const label = document.createElement("div");
        label.className = "type-help-tree-relation";
        label.textContent = relation;
        connector.append(label);
    }
    const childMarker = document.createElement("div");
    childMarker.className = "type-help-tree-child-marker";
    childMarker.textContent = "└─";
    connector.append(childMarker);
    return connector;
}
function renderTypeHelpTree(node) {
    const element = document.createElement("div");
    element.className = "type-help-tree-node";
    const row = document.createElement("div");
    row.className = "type-help-tree-node-row";
    const label = document.createElement("div");
    label.className = `type-help-tree-label type-help-tree-label-${node.kind}`;
    const functionTakesNoArguments = node.kind === "function" && node.label === "function taking no arguments";
    label.textContent = functionTakesNoArguments ? "function" : node.label;
    if (node.typeName)
        label.classList.add("type-help-type");
    row.append(label);
    if (node.children.length === 1 && !functionTakesNoArguments) {
        const child = node.children[0];
        row.append(typeHelpTreeConnector(typeHelpTreeRelation(node, child.relation)), renderTypeHelpTree(child.node));
    }
    element.append(row);
    if (node.children.length > 1 || (functionTakesNoArguments && node.children.length)) {
        const children = document.createElement("div");
        children.className = "type-help-tree-children";
        if (functionTakesNoArguments) {
            const annotationBranch = document.createElement("div");
            annotationBranch.className =
                "type-help-tree-branch type-help-tree-annotation-branch";
            const annotation = document.createElement("span");
            annotation.className = "type-help-tree-annotation";
            annotation.textContent = "(no args)";
            annotationBranch.append(annotation);
            children.append(annotationBranch);
        }
        for (const child of node.children) {
            const branch = document.createElement("div");
            branch.className = "type-help-tree-branch";
            branch.append(typeHelpTreeConnector(typeHelpTreeRelation(node, child.relation)), renderTypeHelpTree(child.node));
            children.append(branch);
        }
        element.append(children);
    }
    return element;
}
function renderExpandedTypeHelp(node, pluralHead = false, kindCounts = new Map()) {
    const element = document.createElement("span");
    element.className = `type-help-phrase type-help-phrase-${node.kind}`;
    const shadeClass = nextTypeHelpShadeClass(node, kindCounts);
    if (shadeClass)
        element.classList.add(shadeClass);
    const label = document.createElement("span");
    label.className = "type-help-phrase-label";
    const displayedLabel = pluralHead ? pluralizeTypeHelpLabel(node) : node.label;
    if (pluralHead && node.typeName && displayedLabel.startsWith(node.label)) {
        label.textContent = node.label;
        const suffixText = displayedLabel.slice(node.label.length);
        if (suffixText) {
            const suffix = document.createElement("span");
            suffix.className = "type-help-plural-suffix";
            suffix.textContent = suffixText;
            label.append(suffix);
        }
    }
    else {
        label.textContent = displayedLabel;
    }
    if (node.typeName)
        label.classList.add("type-help-type");
    element.append(label);
    if (node.kind === "array" && node.children.length === 1) {
        element.append(" ", ...(node.label === "array of unknown length" ? ["containing "] : []), renderExpandedTypeHelp(node.children[0].node, true, kindCounts));
    }
    else if (node.kind === "function") {
        const parameters = node.children.filter((child) => child.relation.startsWith("parameter "));
        const returnType = node.children.find((child) => child.relation === "returns");
        if (parameters.length) {
            parameters.forEach((parameter, index) => {
                const connector = index === 0
                    ? connectorWithArticle("taking", parameter.node)
                    : index === parameters.length - 1
                        ? connectorWithArticle("and", parameter.node)
                        : connectorWithArticle("", parameter.node);
                appendTypeHelpConnector(element, connector);
                element.append(renderExpandedTypeHelp(parameter.node, false, kindCounts));
                if (parameters.length > 2 && index < parameters.length - 1) {
                    appendTypeHelpConnector(element, ",", true);
                }
            });
        }
        if (returnType) {
            const labelAlreadyDescribesArguments = node.label.includes("argument") || node.label.includes("taking");
            appendTypeHelpConnector(element, connectorWithArticle(parameters.length || labelAlreadyDescribesArguments
                ? "and returning"
                : "returning", returnType.node));
            element.append(renderExpandedTypeHelp(returnType.node, false, kindCounts));
        }
    }
    else {
        for (const child of node.children) {
            const isDistributedPointer = pluralHead && node.kind === "pointer" && child.relation === "to";
            if (isDistributedPointer) {
                appendTypeHelpConnector(element, ",", true);
            }
            appendTypeHelpConnector(element, connectorWithArticle(isDistributedPointer ? "each pointing to" : child.relation, child.node));
            element.append(renderExpandedTypeHelp(child.node, false, kindCounts));
        }
    }
    return element;
}
function conciseTypeHelpToken(text, typeName = false) {
    const token = document.createElement("span");
    token.className = "type-help-concise-token";
    token.textContent = text;
    if (typeName)
        token.classList.add("type-help-type");
    return token;
}
function renderConciseTypeHelp(node, kindCounts = new Map()) {
    const element = document.createElement("span");
    element.className = `type-help-phrase type-help-phrase-${node.kind}`;
    const shadeClass = nextTypeHelpShadeClass(node, kindCounts);
    if (shadeClass)
        element.classList.add(shadeClass);
    if (node.kind === "type") {
        element.append(conciseTypeHelpToken(node.label, !!node.typeName));
        return element;
    }
    if (node.kind === "pointer") {
        let pointerCount = 1;
        let tail = node;
        while (tail.children.length === 1
            && tail.children[0].node.kind === "pointer") {
            pointerCount += 1;
            tail = tail.children[0].node;
        }
        element.append(conciseTypeHelpToken("*".repeat(pointerCount)));
        for (const child of tail.children) {
            element.append(renderConciseTypeHelp(child.node, kindCounts));
        }
        return element;
    }
    if (node.kind === "array") {
        const length = /^array of\s+(.+)$/.exec(node.label)?.[1] ?? "?";
        element.append(conciseTypeHelpToken(length === "unknown length" ? "[]" : `[${length}]`));
        for (const child of node.children) {
            element.append(renderConciseTypeHelp(child.node, kindCounts));
        }
        return element;
    }
    const parameters = node.children.filter((child) => child.relation.startsWith("parameter "));
    const returnType = node.children.find((child) => child.relation === "returns");
    const openingParen = conciseTypeHelpToken("(");
    openingParen.classList.add("type-help-concise-paren");
    element.append(openingParen);
    parameters.forEach((parameter, index) => {
        if (index)
            element.append(conciseTypeHelpToken(","));
        element.append(renderConciseTypeHelp(parameter.node, kindCounts));
    });
    if (node.label.includes("variadic")) {
        if (parameters.length)
            element.append(conciseTypeHelpToken(","));
        element.append(conciseTypeHelpToken("…"));
    }
    else if (node.label.includes("unspecified")) {
        element.append(conciseTypeHelpToken("?"));
    }
    const closingParen = conciseTypeHelpToken(")");
    closingParen.classList.add("type-help-concise-paren");
    element.append(closingParen);
    const arrow = conciseTypeHelpToken("→");
    arrow.classList.add("type-help-concise-arrow");
    element.append(arrow);
    if (returnType) {
        element.append(renderConciseTypeHelp(returnType.node, kindCounts));
    }
    return element;
}
function attachTypeHelp(root, typeSelector, typeInfo) {
    const help = String(typeInfo?.help ?? "").trim();
    if (!help)
        return;
    const typeEl = root.querySelector(typeSelector);
    if (!typeEl || typeEl.closest(".type-value-row"))
        return;
    const precedingElement = typeEl.previousElementSibling;
    const typeLabel = precedingElement instanceof HTMLElement && precedingElement.matches(".lbl")
        ? precedingElement
        : null;
    const row = document.createElement("div");
    row.className = "type-value-row";
    typeEl.replaceWith(row);
    row.appendChild(typeEl);
    const helpId = `type-help-${++nextTypeHelpId}`;
    const control = document.createElement("div");
    control.className = "type-help-control";
    const button = document.createElement("button");
    button.className = "type-help-button";
    button.type = "button";
    button.textContent = "?";
    button.setAttribute("aria-label", `Explain the type ${typeEl.textContent?.trim() || "shown"}`);
    button.setAttribute("aria-describedby", helpId);
    button.setAttribute("aria-controls", helpId);
    button.setAttribute("aria-expanded", "false");
    const tooltip = document.createElement("div");
    tooltip.className = "type-help-tooltip";
    tooltip.id = helpId;
    tooltip.setAttribute("role", "tooltip");
    const hoverBridge = document.createElement("div");
    hoverBridge.className = "type-help-hover-bridge";
    hoverBridge.setAttribute("aria-hidden", "true");
    const modeButtons = [];
    let explanation = null;
    let tree = null;
    if (typeInfo?.helpTree) {
        const modeSwitch = document.createElement("div");
        modeSwitch.className = "type-help-mode-switch";
        modeSwitch.setAttribute("role", "group");
        modeSwitch.setAttribute("aria-label", "Type explanation view");
        const modes = [
            { mode: "english", label: "English" },
            { mode: "concise", label: "Concise" },
            { mode: "tree", label: "Tree" },
        ];
        for (const { mode, label } of modes) {
            const modeButton = document.createElement("button");
            modeButton.type = "button";
            modeButton.className = "type-help-mode-button";
            modeButton.textContent = label;
            modeButton.setAttribute("aria-pressed", String(mode === "english"));
            modeSwitch.append(modeButton);
            modeButtons.push({ mode, button: modeButton });
        }
        explanation = document.createElement("div");
        explanation.className = "type-help-explanation";
        explanation.append(renderExpandedTypeHelp(typeInfo.helpTree));
        tooltip.append(modeSwitch, explanation);
        tree = document.createElement("div");
        tree.className = "type-help-tree-view";
        tree.append(renderTypeHelpTree(typeInfo.helpTree));
        tree.hidden = true;
        tooltip.append(tree);
    }
    else {
        tooltip.textContent = help;
    }
    const supportsPopover = typeof tooltip.showPopover === "function";
    if (supportsPopover)
        tooltip.setAttribute("popover", "manual");
    const placeTooltip = () => {
        const buttonRect = button.getBoundingClientRect();
        const typeRect = typeEl.getBoundingClientRect();
        const tooltipRect = tooltip.getBoundingClientRect();
        const viewportPadding = 12;
        const gap = 7;
        const maxLeft = Math.max(viewportPadding, window.innerWidth - tooltipRect.width - viewportPadding);
        const left = Math.min(maxLeft, Math.max(viewportPadding, buttonRect.right - tooltipRect.width));
        const anchorTop = Math.min(buttonRect.top, typeRect.top);
        const anchorBottom = Math.max(buttonRect.bottom, typeRect.bottom);
        let top = anchorBottom + gap;
        if (top + tooltipRect.height > window.innerHeight - viewportPadding &&
            anchorTop - tooltipRect.height - gap >= viewportPadding) {
            top = anchorTop - tooltipRect.height - gap;
        }
        top = Math.min(Math.max(viewportPadding, top), Math.max(viewportPadding, window.innerHeight - tooltipRect.height - viewportPadding));
        tooltip.style.left = `${left}px`;
        tooltip.style.top = `${Math.round(top)}px`;
        const tooltipBottom = top + tooltipRect.height;
        const belowButton = top >= buttonRect.bottom;
        const aboveButton = tooltipBottom <= buttonRect.top;
        const bridgeTop = belowButton
            ? buttonRect.bottom
            : aboveButton
                ? tooltipBottom
                : 0;
        const bridgeBottom = belowButton
            ? top
            : aboveButton
                ? buttonRect.top
                : 0;
        const bridgeLeft = Math.min(buttonRect.left, left);
        const bridgeRight = Math.max(buttonRect.right, left + tooltipRect.width);
        const bridgeHeight = Math.max(0, bridgeBottom - bridgeTop + 1);
        hoverBridge.classList.toggle("is-active", bridgeHeight > 0);
        hoverBridge.style.left = `${bridgeLeft}px`;
        hoverBridge.style.top = `${bridgeTop}px`;
        hoverBridge.style.width = `${bridgeRight - bridgeLeft}px`;
        hoverBridge.style.height = `${bridgeHeight}px`;
    };
    const selectTypeHelpMode = (selectedMode) => {
        if (!explanation || !tree || !typeInfo?.helpTree)
            return;
        for (const { mode, button: modeButton } of modeButtons) {
            modeButton.setAttribute("aria-pressed", String(mode === selectedMode));
        }
        const showTree = selectedMode === "tree";
        explanation.hidden = showTree;
        tree.hidden = !showTree;
        if (!showTree) {
            const concise = selectedMode === "concise";
            explanation.classList.toggle("is-concise", concise);
            explanation.replaceChildren(concise
                ? renderConciseTypeHelp(typeInfo.helpTree)
                : renderExpandedTypeHelp(typeInfo.helpTree));
        }
        placeTooltip();
    };
    for (const { mode, button: modeButton } of modeButtons) {
        modeButton.addEventListener("click", () => selectTypeHelpMode(mode));
    }
    const showTooltip = () => {
        if (supportsPopover) {
            if (!tooltip.matches(":popover-open"))
                tooltip.showPopover();
        }
        else {
            tooltip.classList.add("is-open");
        }
        button.setAttribute("aria-expanded", "true");
        placeTooltip();
    };
    const closeTooltip = () => {
        if (supportsPopover) {
            if (tooltip.matches(":popover-open"))
                tooltip.hidePopover();
        }
        else {
            tooltip.classList.remove("is-open");
        }
        button.setAttribute("aria-expanded", "false");
        hoverBridge.classList.remove("is-active");
    };
    const hideTooltip = () => {
        if (control.matches(":hover")
            || tooltip.matches(":hover")
            || control.contains(document.activeElement)) {
            return;
        }
        closeTooltip();
    };
    let openBeforePointerDown = false;
    button.addEventListener("pointerdown", () => {
        openBeforePointerDown = supportsPopover
            ? tooltip.matches(":popover-open")
            : tooltip.classList.contains("is-open");
    });
    button.addEventListener("click", () => {
        if (openBeforePointerDown) {
            closeTooltip();
        }
        else {
            showTooltip();
        }
    });
    button.addEventListener("mouseenter", showTooltip);
    button.addEventListener("mouseleave", () => window.setTimeout(hideTooltip));
    button.addEventListener("focus", showTooltip);
    button.addEventListener("blur", () => window.setTimeout(hideTooltip));
    tooltip.addEventListener("mouseleave", () => window.setTimeout(hideTooltip));
    tooltip.addEventListener("focusout", () => window.setTimeout(hideTooltip));
    hoverBridge.addEventListener("mouseleave", () => window.setTimeout(hideTooltip));
    control.append(button, hoverBridge, tooltip);
    if (typeLabel) {
        typeLabel.classList.add("type-help-label");
        typeLabel.appendChild(control);
    }
    else {
        row.appendChild(control);
    }
}
function queryElement(selector, root = document) {
    return (root?.querySelector?.(selector) ?? null);
}
function queryRole(role, root = document) {
    return queryElement(`[data-role="${role}"]`, root);
}
function clearNode(node) {
    node?.replaceChildren();
}
function el(html) {
    const t = document.createElement("template");
    t.innerHTML = html.trim();
    return t.content.firstElementChild;
}
function txt(n) {
    return (n?.textContent || "").trim();
}
function onDomReady(fn, { once = true } = {}) {
    if (document.readyState === "loading") {
        document.addEventListener("DOMContentLoaded", () => {
            fn();
        }, once ? { once: true } : undefined);
        return;
    }
    fn();
}
function disableAutoText(el) {
    if (!el || el.nodeType !== 1)
        return;
    el.setAttribute("autocapitalize", "off");
    el.setAttribute("autocorrect", "off");
    el.setAttribute("autocomplete", "off");
    el.setAttribute("spellcheck", "false");
}
function applyAutoTextDefaults(root = document) {
    root
        .querySelectorAll('input[type="text"], input:not([type]), textarea, [contenteditable="true"]')
        .forEach((el) => disableAutoText(el));
}
const MOBILE_MEDIA_QUERY = "(max-width: 900px)";
function isMobileViewport() {
    return window.matchMedia && window.matchMedia(MOBILE_MEDIA_QUERY).matches;
}
const DEFAULT_NAV_ITEMS = NAV_ITEMS;
function resolveNavItems(items) {
    return items?.length ? items : DEFAULT_NAV_ITEMS;
}
function normalizeNavHref(href = "") {
    const clean = String(href || "")
        .split("#")[0]
        .split("?")[0];
    const parts = clean.split("/").filter(Boolean);
    return parts[parts.length - 1] || "index.html";
}
function getNavLabelForHref(href) {
    if (!href)
        return null;
    const target = normalizeNavHref(href);
    const match = DEFAULT_NAV_ITEMS.find((item) => normalizeNavHref(item?.href || "") === target);
    const label = match?.label || "";
    if (!label)
        return null;
    return label.replace(/^\d+\.\s*/, "");
}
function getPreviousNavHref(href) {
    const current = normalizeNavHref(href || currentNavHref());
    const index = DEFAULT_NAV_ITEMS.findIndex((item) => normalizeNavHref(item?.href || "") === current);
    return index > 1 ? DEFAULT_NAV_ITEMS[index - 1]?.href ?? null : null;
}
function currentNavHref() {
    const pathname = window.location?.pathname || "";
    const cleaned = normalizeNavHref(pathname);
    return cleaned || "index.html";
}
function resolveActiveNavItem(items = DEFAULT_NAV_ITEMS, activeHref) {
    const list = resolveNavItems(items);
    const current = normalizeNavHref(activeHref || currentNavHref());
    return list.find((item) => normalizeNavHref(item?.href || "") === current);
}
function buildNav(items = DEFAULT_NAV_ITEMS, { activeHref } = {}) {
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
function findExistingLayoutNodes(wrap) {
    const nav = (wrap?.querySelector("nav.tabs") ||
        document.querySelector("nav.tabs"));
    const main = (wrap?.querySelector(".main") ||
        document.querySelector(".main"));
    return { nav, main };
}
function ensureWrapConnected(wrap, nav, main) {
    if (wrap.isConnected)
        return;
    const mount = document.body;
    const firstScript = mount.querySelector("script");
    const anchor = main.parentElement === mount ? main : nav.parentElement === mount ? nav : null;
    if (anchor)
        mount.insertBefore(wrap, anchor);
    else if (firstScript)
        mount.insertBefore(wrap, firstScript);
    else
        mount.appendChild(wrap);
}
function updateSidebarToggleLabel(btn) {
    const hidden = document.body.classList.contains("sidebar-collapsed");
    const label = hidden ? "Show sidebar" : "Hide sidebar";
    btn.classList.toggle("is-expanded", !hidden);
    btn.setAttribute("aria-label", label);
    btn.setAttribute("aria-expanded", hidden ? "false" : "true");
    const sr = btn.querySelector(".sr-only");
    if (sr)
        sr.textContent = label;
}
function updateSidebarQueryParam() {
    const hidden = document.body.classList.contains("sidebar-collapsed");
    const params = new URLSearchParams(window.location.search);
    params.set("sidebar", hidden ? "0" : "1");
    const query = params.toString();
    const next = `${window.location.pathname}?${query}${window.location.hash}`;
    window.history.replaceState(null, "", next);
}
function ensureSidebarControls(wrap, nav) {
    if (!nav.id)
        nav.id = "sidebar";
    let sidebarWrap = wrap.querySelector(".sidebar-wrap");
    if (!sidebarWrap) {
        sidebarWrap = document.createElement("div");
        sidebarWrap.className = "sidebar-wrap";
        wrap.insertBefore(sidebarWrap, wrap.firstChild);
    }
    if (nav.parentElement !== sidebarWrap) {
        sidebarWrap.appendChild(nav);
    }
    let btn = sidebarWrap.querySelector(".sidebar-toggle");
    if (!btn) {
        btn = el('<button type="button" class="sidebar-toggle"><span class="hamburger" aria-hidden="true"><span></span><span></span><span></span></span><span class="sr-only">Toggle sidebar</span></button>');
        sidebarWrap.insertBefore(btn, sidebarWrap.firstChild);
    }
    else if (btn.parentElement !== sidebarWrap) {
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
function ensureBaseLayout({ navItems, activeHref, } = {}) {
    let wrap = document.querySelector(".wrap");
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
        wrap: wrap,
        nav: nav,
        main: main,
    };
}
function syncDocumentTitleFromNav(prefix = "C Boxes") {
    const activeItem = resolveActiveNavItem();
    const resolvedTitle = String(activeItem?.label || "").trim();
    document.title = resolvedTitle ? `${prefix} - ${resolvedTitle}` : prefix;
    return resolvedTitle;
}
function ensurePanelizedMain(title = "") {
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
function centerActiveNavItem(nav) {
    const active = nav.querySelector("a.active");
    if (!active)
        return;
    const activeCenter = active.offsetTop + active.offsetHeight / 2;
    const target = activeCenter - nav.clientHeight / 2;
    const maxScroll = Math.max(0, nav.scrollHeight - nav.clientHeight);
    nav.scrollTop = Math.max(0, Math.min(maxScroll, target));
}
function clampLineIndex(index, maxIndex) {
    return Math.max(0, Math.min(maxIndex, index));
}
function normalizeLineRange(range, maxIndex) {
    const startRaw = Number(Array.isArray(range) ? range[0] : range.start);
    const endRaw = Number(Array.isArray(range) ? range[1] : range.end);
    const start = clampLineIndex(Math.min(startRaw, endRaw), maxIndex);
    const end = clampLineIndex(Math.max(startRaw, endRaw), maxIndex);
    return [start, end];
}
function renderCodePane(root, lines, boundary, opts = {}) {
    clearNode(root);
    const code = el('<div class="codecol"></div>');
    if (opts.progress)
        code.classList.add("has-progress");
    if (opts.boundaryTargets)
        code.classList.add("boundary-targets");
    root.appendChild(code);
    const addBoundary = (boundaryIndex, selectable = false) => {
        const node = el('<div class="boundary"></div>');
        if (selectable) {
            node.classList.add("selectable");
            if (typeof boundaryIndex === "number") {
                node.dataset.boundary = String(boundaryIndex);
                const selected = opts.selectedBoundary != null &&
                    Number(opts.selectedBoundary) === boundaryIndex;
                if (selected)
                    node.classList.add("selected");
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
    let progressRangeStart = null;
    let progressRangeEnd = null;
    let strikeRanges = [];
    const strikeFragmentsByLine = new Map();
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
            }
            else if (!opts.suppressProgressMid && progressRangeStart != null) {
                progressIndex = progressRangeStart;
            }
        }
        else if (typeof opts.progressIndex === "number") {
            progressIndex = Math.max(0, Math.min(lines.length - 1, opts.progressIndex));
        }
        else if (!opts.suppressProgressMid && boundary > 0) {
            progressIndex = boundary - 1;
        }
    }
    const appendStrike = (range) => {
        const normalized = normalizeLineRange(range, Math.max(0, lines.length - 1));
        strikeRanges.push(normalized);
    };
    if (opts.strikeRange)
        appendStrike(opts.strikeRange);
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
            if (end < start)
                [start, end] = [end, start];
            if (end <= start)
                return;
            const list = strikeFragmentsByLine.get(line) || [];
            list.push({ start, end });
            strikeFragmentsByLine.set(line, list);
        });
    }
    if (doneBoundary === 0 && !hideBoundary)
        addBoundary();
    if (selectableBoundaries && selectableBoundaries.has(0)) {
        addBoundary(0, true);
    }
    for (let i = 0; i < lines.length; i++) {
        const lr = el('<div class="line"></div>');
        const ln = el(`<div class="ln">${i + 1}</div>`);
        const src = el('<div class="src"></div>');
        const rawLine = lines[i];
        const inStrikeRange = strikeRanges.some(([start, end]) => i >= start && i <= end);
        const fragments = (strikeFragmentsByLine.get(i) || []).slice();
        if (inStrikeRange && rawLine.length > 0) {
            fragments.push({ start: 0, end: rawLine.length });
        }
        const mergedFragments = [];
        if (fragments.length) {
            const sorted = fragments
                .slice()
                .sort((a, b) => a.start - b.start || a.end - b.end);
            sorted.forEach(({ start, end }) => {
                if (!mergedFragments.length) {
                    mergedFragments.push({ start, end });
                    return;
                }
                const last = mergedFragments[mergedFragments.length - 1];
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
                    src.appendChild(document.createTextNode(rawLine.slice(cursor, start)));
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
        }
        else {
            src.textContent = rawLine;
        }
        if (i < doneBoundary)
            lr.classList.add("done");
        const inProgressRange = progressRangeStart !== null &&
            progressRangeEnd !== null &&
            i >= progressRangeStart &&
            i <= progressRangeEnd;
        if (inProgressRange)
            lr.classList.add("progress-range");
        if (inStrikeRange && !hasFragments)
            lr.classList.add("skipped");
        if (i === progressIndex)
            lr.classList.add("progress-mid");
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
function vbox({ address = "—", type = "int", value = "", name = "", editable = false, allowNameEdit = false, allowTypeEdit = false, showDoubleExact = false, displayValue = null, exactValue = null, typeInfo = null, aliases = [], stateName = null, } = {}) {
    const isFloatingScalar = typeInfo?.kind === "floating";
    const rawValue = value ?? "";
    const emptyDisplay = rawValue === "";
    const renderedValue = emptyDisplay
        ? ""
        : displayValue ?? normalizeZeroDisplay(rawValue);
    const resolvedName = name ?? "";
    const namesList = resolvedName ? [resolvedName] : [""];
    const valueClasses = `value ${editable ? "editable" : ""} ${emptyDisplay ? "placeholder muted" : ""}`;
    const typeClasses = `type ${allowTypeEdit ? "editable" : ""}`;
    const nameClasses = `name-tag ${editable ? "editable" : ""}`;
    const listClasses = "name-list";
    const nameTags = namesList
        .map((n) => {
        const cls = namesList.length > 1 ? `${nameClasses}` : `${nameClasses} single`;
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
          <div class="${valueClasses}">${renderedValue}</div>
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
    if (stateName)
        node.dataset.stateName = stateName;
    const valueEl = node.querySelector(".value");
    if (typeInfo)
        node.dataset.typeInfo = JSON.stringify(typeInfo);
    attachTypeHelp(node, ".type", typeInfo);
    if (aliases.length)
        node.dataset.aliases = JSON.stringify(aliases);
    if (displayValue != null)
        node.dataset.displayValue = displayValue;
    if (exactValue != null)
        node.dataset.exactValue = exactValue;
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
            }
            else {
                valueEl.classList.remove("placeholder", "muted");
                delete valueEl.dataset.empty;
            }
        });
        if (emptyDisplay) {
            valueEl.dataset.empty = "true";
        }
        if (allowTypeEdit) {
            const typeEl = node.querySelector(".type");
            if (typeEl) {
                typeEl.setAttribute("contenteditable", "true");
                typeEl.classList.add("editable");
                disableAutoText(typeEl);
            }
        }
        node.querySelectorAll(".name-text").forEach((el) => {
            if (!allowNameEdit || !(el instanceof HTMLElement))
                return;
            el.setAttribute("contenteditable", "true");
            el.classList.add("editable");
            disableAutoText(el);
        });
    }
    const toggleEl = node.querySelector(".double-toggle");
    if (valueEl && isFloatingScalar && !emptyDisplay && !editable) {
        const shortText = displayValue ?? normalizeZeroDisplay(rawValue);
        const exactText = exactValue ?? shortText;
        if (shortText) {
            valueEl.dataset.doubleValue = rawValue;
            node.dataset.doubleDisplay = showDoubleExact ? "exact" : "short";
            const approx = shortText !== exactText;
            valueEl.textContent = showDoubleExact ? exactText : shortText;
            if (approx && !showDoubleExact)
                valueEl.dataset.doubleApprox = "true";
            else
                delete valueEl.dataset.doubleApprox;
            if (toggleEl) {
                toggleEl.textContent = showDoubleExact ? "short" : "exact";
                toggleEl.setAttribute("aria-pressed", showDoubleExact ? "true" : "false");
                if (approx) {
                    toggleEl.classList.remove("hidden");
                }
                else {
                    toggleEl.classList.add("hidden");
                }
                toggleEl.addEventListener("click", (e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    const nextExact = node.dataset.doubleDisplay !== "exact";
                    node.dataset.doubleDisplay = nextExact ? "exact" : "short";
                    valueEl.textContent = nextExact ? exactText : shortText;
                    if (approx && !nextExact)
                        valueEl.dataset.doubleApprox = "true";
                    else
                        delete valueEl.dataset.doubleApprox;
                    toggleEl.textContent = nextExact ? "short" : "exact";
                    toggleEl.setAttribute("aria-pressed", nextExact ? "true" : "false");
                    if (approx) {
                        toggleEl.classList.remove("hidden");
                    }
                    else {
                        toggleEl.classList.add("hidden");
                    }
                });
            }
        }
    }
    else if (toggleEl) {
        toggleEl.classList.add("hidden");
    }
    return node;
}
function disableBoxEditing(root) {
    if (!root)
        return;
    root
        .querySelectorAll(".value.editable, .type.editable, .name-text.editable, .array-col-value.editable")
        .forEach((el) => {
        el.removeAttribute("contenteditable");
        el.classList.remove("editable");
    });
    root.classList.remove("is-editable");
}
function removeBoxDeleteButtons(root) {
    const scope = root || document;
    scope
        .querySelectorAll(".vbox .delete, .arraybox .delete, .aggregatebox .delete")
        .forEach((btn) => btn.remove());
}
function readBoxState(root) {
    const el = root;
    const names = [...root.querySelectorAll(".name-text")]
        .map((el) => txt(el))
        .filter(Boolean);
    const valEl = root.querySelector(".value");
    const valText = txt(valEl);
    const typeText = txt(root.querySelector(".type"));
    const placeholderEmpty = valEl?.classList?.contains("placeholder") && valText === "";
    const fallbackRawValue = placeholderEmpty ? "" : valText;
    const storedRawValue = valEl instanceof HTMLElement ? valEl.dataset.rawValue : undefined;
    const rawValue = storedRawValue !== undefined ? storedRawValue : fallbackRawValue;
    let value = normalizeZeroDisplay(rawValue);
    const valueEditable = valEl instanceof HTMLElement && valEl.isContentEditable;
    if (valEl instanceof HTMLElement && !valueEditable) {
        const stored = valEl.dataset.doubleValue;
        if (stored != null && stored !== "")
            value = stored;
    }
    const allowDelete = el.dataset.allowDelete === "true" || !!root.querySelector(".delete");
    return {
        address: txt(root.querySelector(".address")),
        type: typeText,
        value,
        displayValue: el.dataset.displayValue ?? null,
        exactValue: el.dataset.exactValue ?? null,
        rawValue,
        name: el.dataset.stateName || names[0] || "",
        names,
        allowNameEdit: !!root.querySelector(".name-text[contenteditable]"),
        allowTypeEdit: !!root.querySelector(".type[contenteditable]"),
        allowDelete,
        aliases: parseStoredAliases(el.dataset.aliases),
        typeInfo: parseStoredTypeInfo(el.dataset.typeInfo),
        showDoubleExact: el.dataset.doubleDisplay === "exact",
        dynamicAddress: el.dataset.dynamicAddress === "true",
        defaultAddressType: el.dataset.defaultAddressType ?? null,
        expectedAddress: el.dataset.expectedAddress ?? null,
        expectedAddressType: el.dataset.expectedAddressType ?? null,
    };
}
function parseStoredAliases(raw) {
    if (!raw)
        return [];
    try {
        const parsed = JSON.parse(raw);
        return Array.isArray(parsed) ? parsed.map((value) => String(value)) : [];
    }
    catch {
        return [];
    }
}
function parseStoredTypeInfo(raw) {
    if (!raw)
        return null;
    try {
        const parsed = JSON.parse(raw);
        return parsed && typeof parsed === "object" ? parsed : null;
    }
    catch {
        return null;
    }
}
function boxAddress(box) {
    const raw = box?.address ?? "";
    return raw.trim();
}
function collectStageBoxes(root) {
    return [...root.querySelectorAll(".vbox")]
        .map((node) => {
        const box = readBoxState(node);
        box.node = node;
        return box;
    });
}
function updateOtherNamesList(node, aliases, showAliases) {
    const listInner = node.querySelector(".name-list-inner");
    if (!listInner)
        return;
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
    const hasAliases = showAliases && aliases.length > 0;
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
    if (label)
        label.textContent = hasAliases ? "names" : "name";
    const main = node.querySelector(".vbox-main");
    const listWidth = listInner.getBoundingClientRect().width;
    const mainWidth = main?.getBoundingClientRect().width ?? 0;
    const overhang = hasAliases
        ? Math.max(0, Math.ceil((listWidth - mainWidth) / 2))
        : 0;
    node.style.setProperty("--name-overhang", `${overhang}px`);
}
function ensureOtherNamesToggle(node, onToggle) {
    const stack = node.querySelector(".name-stack");
    const label = node.querySelector(".lbl-name");
    if (!stack || !label)
        return null;
    let btn = node.querySelector(".other-names-toggle");
    if (!btn) {
        btn = document.createElement("button");
        btn.type = "button";
        btn.className = "other-names-toggle";
        btn.textContent = "Show aliases";
    }
    if (!btn.dataset.bound) {
        btn.dataset.bound = "true";
        btn.addEventListener("click", (event) => {
            event.preventDefault();
            onToggle(node);
        });
    }
    if (btn.parentElement !== stack || btn.nextElementSibling !== label) {
        stack.insertBefore(btn, label);
    }
    return btn;
}
function applyOtherNames(root, opts = {}) {
    if (!root)
        return;
    const { onToggle = null, shownAddrs = null, sourceBoxes = null, cleanupShownAddrs = true, } = opts;
    const boxes = collectStageBoxes(root);
    const currentAddrs = new Set(boxes.map((box) => boxAddress(box)));
    const aliasSource = Array.isArray(sourceBoxes) && sourceBoxes.length ? sourceBoxes : boxes;
    const aliasesByAddr = new Map(aliasSource.map((box) => [boxAddress(box), box.aliases ?? []]));
    const useShownSet = shownAddrs !== null;
    const getShown = (node, addr) => useShownSet ? shownAddrs.has(addr) : node.dataset.otherNames === "on";
    const setShown = (node, addr, value) => {
        if (useShownSet) {
            if (value)
                shownAddrs.add(addr);
            else
                shownAddrs.delete(addr);
        }
        node.dataset.otherNames = value ? "on" : "off";
    };
    boxes.forEach((box) => {
        const node = box.node;
        if (!node)
            return;
        const addr = boxAddress(box);
        const baseName = String(box.name || "").trim();
        const aliases = aliasesByAddr.get(addr) ?? [];
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
            if (!currentAddrs.has(addr))
                shownAddrs.delete(addr);
        });
    }
}
function makeAnswerBox({ name = "", type = "", value = "", address = null, editable = true, deletable = editable, allowNameEdit = null, allowTypeEdit = null, showDoubleExact = null, displayValue = null, exactValue = null, typeInfo = null, aliases = [], stateName = null, } = {}) {
    const resolvedAddr = address == null ? "—" : String(address);
    const resolvedNameEdit = allowNameEdit !== null && allowNameEdit !== undefined ? allowNameEdit : !name;
    const resolvedTypeEdit = allowTypeEdit !== null && allowTypeEdit !== undefined ? allowTypeEdit : !type;
    const node = vbox({
        address: resolvedAddr,
        type,
        value,
        name,
        editable,
        allowNameEdit: resolvedNameEdit,
        allowTypeEdit: resolvedTypeEdit,
        showDoubleExact: showDoubleExact ?? false,
        displayValue,
        exactValue,
        typeInfo,
        aliases,
        stateName,
    });
    if (deletable) {
        const del = el('<button class="delete" title="delete">×</button>');
        node.appendChild(del);
        del.addEventListener("click", () => node.remove());
    }
    return node;
}
function normalizeArrayDims(value) {
    if (!Array.isArray(value))
        return [];
    const out = [];
    for (const raw of value) {
        const num = Math.floor(Number(raw));
        if (!Number.isFinite(num) || num <= 0)
            return [];
        out.push(num);
    }
    return out;
}
function parseArrayElementName(name) {
    const match = /^([A-Za-z_][A-Za-z0-9_]*)((?:\[\d+\])+)$/.exec(String(name || ""));
    if (!match)
        return null;
    const baseName = match[1] || "";
    const suffix = match[2] || "";
    if (!baseName || !suffix)
        return null;
    const indices = [];
    const rx = /\[(\d+)\]/g;
    let m;
    while ((m = rx.exec(suffix)) != null) {
        const value = Number(m[1]);
        if (!Number.isFinite(value) || value < 0)
            return null;
        indices.push(value);
    }
    if (!indices.length)
        return null;
    return { baseName, indices };
}
function arrayElementName(name, indices) {
    return `${name}${indices.map((index) => `[${index}]`).join("")}`;
}
function inferArrayShapeFromEntries(entries) {
    if (!entries.length)
        return [];
    const dims = entries[0]?.indices.length || 0;
    if (!dims)
        return [];
    const shape = new Array(dims).fill(0);
    for (const entry of entries) {
        if (entry.indices.length !== dims)
            return [];
        for (let i = 0; i < dims; i++) {
            const value = entry.indices[i];
            if (value < 0)
                return [];
            shape[i] = Math.max(shape[i], value + 1);
        }
    }
    return shape.every((dim) => dim > 0) ? shape : [];
}
function arrayLinearIndex(indices, shape) {
    if (indices.length !== shape.length)
        return null;
    let linear = 0;
    let stride = 1;
    for (let i = shape.length - 1; i >= 0; i--) {
        const idx = indices[i];
        const dim = shape[i];
        if (idx < 0 || idx >= dim)
            return null;
        linear += idx * stride;
        stride *= dim;
    }
    return linear;
}
function arrayElementCount(shape) {
    return shape.reduce((acc, dim) => acc * dim, 1);
}
function sameDims(a, b) {
    if (a.length !== b.length)
        return false;
    for (let i = 0; i < a.length; i++) {
        if (a[i] !== b[i])
            return false;
    }
    return true;
}
function groupStateObjects(boxes) {
    const arrayCandidates = new Map();
    const arrayRoots = new Map();
    const scalars = [];
    boxes.forEach((box, index) => {
        let baseName = "";
        let indices = [];
        const metaShape = normalizeArrayDims(box.arrayShape);
        const metaIndices = Array.isArray(box.arrayIndices)
            ? box.arrayIndices
                .map((value) => Math.floor(Number(value)))
                .filter((value) => Number.isFinite(value) && value >= 0)
            : [];
        if (!box.arrayRoot && metaShape.length > 0 && metaIndices.length === 0) {
            const rootName = String(box.name || "").trim();
            if (rootName && !arrayRoots.has(rootName)) {
                arrayRoots.set(rootName, { box, shape: metaShape, index });
                return;
            }
        }
        if (box.arrayRoot && Array.isArray(box.arrayIndices) && box.arrayIndices.length > 0) {
            baseName = String(box.arrayRoot);
            indices = metaIndices;
        }
        else {
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
    const out = [...scalars];
    const consumedRoots = new Set();
    for (const [baseName, list] of arrayCandidates.entries()) {
        if (!list.length)
            continue;
        const sorted = list.slice().sort((a, b) => a.index - b.index);
        const first = sorted[0];
        const root = arrayRoots.get(baseName) || null;
        const shapes = sorted
            .map((item) => item.shape)
            .filter((shape) => shape.length > 0);
        let shape = root?.shape.length
            ? root.shape.slice()
            : shapes.length
                ? shapes[0].slice()
                : inferArrayShapeFromEntries(sorted);
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
        const entries = [];
        const seenLinear = new Set();
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
            index: root?.index ?? first.index,
            address: root?.box.address ?? first.box.address ?? null,
            elementType,
            type: String(root?.box.type ?? "").trim(),
            typeInfo: root?.box.typeInfo ?? first.box.typeInfo ?? null,
            entries,
            allowDelete: root?.box.allowDelete !== null && root?.box.allowDelete !== undefined
                ? !!root.box.allowDelete
                : entries.every((entry) => !!entry.box.allowDelete),
        });
        if (root)
            consumedRoots.add(baseName);
    }
    for (const [baseName, root] of arrayRoots.entries()) {
        if (!consumedRoots.has(baseName)) {
            out.push({ kind: "scalar", box: root.box, index: root.index });
        }
    }
    out.sort((a, b) => a.index - b.index);
    return out;
}
function makeSubarrayRootName(baseName, prefix) {
    return `${baseName}${prefix.map((index) => `[${index}]`).join("")}`;
}
function buildSyntheticSubarrayBoxes(sourceEntries, baseName, prefix, shape) {
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
function findSubarrayBoxesInGroup(group, expectedShape, baseAddress) {
    const rank = group.shape.length;
    if (!rank || !expectedShape.length || expectedShape.length > rank)
        return null;
    if (expectedShape.length === rank) {
        if (!sameDims(group.shape, expectedShape))
            return null;
        const firstAddr = String(group.entries[0]?.box.address ?? "").trim();
        if (firstAddr !== baseAddress)
            return null;
        return group.entries.map((entry) => entry.box);
    }
    const prefixLen = rank - expectedShape.length;
    if (!sameDims(group.shape.slice(prefixLen), expectedShape))
        return null;
    const start = group.entries.find((entry) => {
        if (entry.indices.length !== rank)
            return false;
        if (String(entry.box.address ?? "").trim() !== baseAddress)
            return false;
        for (let i = prefixLen; i < rank; i++) {
            if ((entry.indices[i] ?? -1) !== 0)
                return false;
        }
        return true;
    });
    if (!start)
        return null;
    const prefix = start.indices.slice(0, prefixLen);
    const subset = group.entries.filter((entry) => {
        if (entry.indices.length !== rank)
            return false;
        for (let i = 0; i < prefixLen; i++) {
            if (entry.indices[i] !== prefix[i])
                return false;
        }
        return true;
    });
    if (subset.length !== arrayElementCount(expectedShape))
        return null;
    const rebased = subset.map((entry) => ({
        entry,
        suffix: entry.indices.slice(prefixLen),
    }));
    const seen = new Set();
    for (const item of rebased) {
        const linear = arrayLinearIndex(item.suffix, expectedShape);
        if (linear == null || seen.has(linear))
            return null;
        seen.add(linear);
    }
    if (seen.size !== arrayElementCount(expectedShape))
        return null;
    const sortedSubset = rebased
        .slice()
        .sort((left, right) => (arrayLinearIndex(left.suffix, expectedShape) || 0) -
        (arrayLinearIndex(right.suffix, expectedShape) || 0))
        .map((item) => item.entry);
    return buildSyntheticSubarrayBoxes(sortedSubset, group.name, prefix, expectedShape);
}
function findArrayObjectBoxesForResult(result, state) {
    if (!result || !Array.isArray(state) || !state.length)
        return null;
    const expectedShape = normalizeArrayDims(result.typeInfo?.arrayShape);
    if (!expectedShape.length)
        return null;
    const baseAddress = String(result.value ?? "").trim() || String(result.address ?? "").trim();
    if (!baseAddress)
        return null;
    const grouped = groupStateObjects(state);
    const expectedType = String(result.type || "").trim();
    const withResultTypeInfo = (boxes) => boxes.map((box) => ({ ...box, typeInfo: result.typeInfo ?? box.typeInfo ?? null }));
    for (const item of grouped) {
        if (item.kind !== "array")
            continue;
        const isRootTypeMatch = item.type === expectedType;
        if (isRootTypeMatch) {
            const firstAddr = String(item.entries[0]?.box.address ?? "").trim();
            if (firstAddr === baseAddress) {
                return withResultTypeInfo(item.entries.map((entry) => entry.box));
            }
        }
        const subset = findSubarrayBoxesInGroup(item, expectedShape, baseAddress);
        if (subset)
            return withResultTypeInfo(subset);
    }
    return null;
}
function makeArrayBox(group, opts) {
    const { editable, deletable, displayName = group.name } = opts;
    const typeText = group.type
        || `${group.elementType}${group.shape.map((d) => `[${d}]`).join("")}`;
    const firstAddress = String(group.address ?? group.entries[0]?.box.address ?? "—");
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
    node.querySelector(".array-address").textContent = firstAddress;
    node.querySelector(".array-type").textContent = typeText;
    attachTypeHelp(node, ".array-type", group.typeInfo);
    node.querySelector(".array-name").textContent = displayName;
    const valuesWrap = node.querySelector(".array-values");
    if (!valuesWrap)
        return node;
    const lastDim = group.shape[group.shape.length - 1];
    let currentRowKey = "";
    let rowEl = null;
    let rowValuesEl = null;
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
        if (!rowValuesEl)
            continue;
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
        if (empty)
            valueEl.classList.add("placeholder", "muted");
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
                }
                else {
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
            node.remove();
        });
        node.dataset.allowDelete = "true";
    }
    return node;
}
function aggregatePath(box) {
    return Array.isArray(box.aggregatePath)
        ? box.aggregatePath.map((part) => String(part))
        : [];
}
function pathStartsWith(path, prefix) {
    if (path.length < prefix.length)
        return false;
    return prefix.every((part, index) => path[index] === part);
}
function appendScalarStateBox(container, box, opts) {
    const allowDelete = box.allowDelete !== null && box.allowDelete !== undefined
        ? !!box.allowDelete
        : opts.deletable;
    const node = makeAnswerBox({
        name: opts.displayName ?? box.name,
        stateName: opts.displayName ? box.name : null,
        type: box.type,
        value: box.rawValue ?? box.value,
        address: box.address ?? null,
        editable: opts.editable,
        deletable: allowDelete,
        allowNameEdit: opts.allowNameEdit ?? box.allowNameEdit,
        allowTypeEdit: opts.allowTypeEdit ?? box.allowTypeEdit,
        showDoubleExact: box.showDoubleExact ?? null,
        displayValue: box.displayValue ?? null,
        exactValue: box.exactValue ?? null,
        typeInfo: box.typeInfo ?? null,
        aliases: box.aliases ?? [],
    });
    if (allowDelete)
        node.dataset.allowDelete = "true";
    if (box.dynamicAddress)
        node.dataset.dynamicAddress = "true";
    if (box.defaultAddressType)
        node.dataset.defaultAddressType = box.defaultAddressType;
    if (box.expectedAddress)
        node.dataset.expectedAddress = box.expectedAddress;
    if (box.expectedAddressType) {
        node.dataset.expectedAddressType = box.expectedAddressType;
    }
    if ((box.value ?? "") === "") {
        node.querySelector(".value")?.classList.add("placeholder", "muted");
    }
    container.appendChild(node);
}
function makeAggregateBox(root, descendants, opts) {
    const displayName = opts.displayName ?? root.name;
    const kind = root.aggregateKind === "union" ? "union" : "struct";
    const prefix = aggregatePath(root);
    const node = el(`
    <div class="aggregatebox ${opts.editable ? "is-editable" : ""}">
      <div class="aggregatebox-main">
        <div class="aggregatebox-address-row">
          <div class="lbl lbl-aggregate-addr">address</div>
          <div class="aggregate-address"></div>
        </div>
        <div class="aggregate-members-wrap">
          <div class="aggregate-label"></div>
          <div class="aggregate-status"></div>
          <div class="aggregate-members"></div>
        </div>
        <div class="aggregate-name-stack">
          <div class="aggregate-name"></div>
          <div class="lbl lbl-aggregate-name">name</div>
        </div>
      </div>
      <div class="aggregatebox-meta">
        <div class="lbl lbl-aggregate-type">type</div>
        <div class="aggregate-type"></div>
      </div>
    </div>
  `);
    node.dataset.aggregateName = root.name;
    node.querySelector(".aggregate-address").textContent = String(root.address ?? "—");
    node.querySelector(".aggregate-name").textContent = displayName;
    node.querySelector(".aggregate-type").textContent = root.type;
    attachTypeHelp(node, ".aggregate-type", root.typeInfo);
    node.querySelector(".aggregate-label").textContent =
        kind === "union" ? "active member" : "members";
    const status = String(root.displayValue ?? root.value ?? "").trim();
    const statusNode = node.querySelector(".aggregate-status");
    statusNode.textContent = status;
    statusNode.classList.toggle("hidden", !status);
    const membersNode = node.querySelector(".aggregate-members");
    const directMembers = descendants
        .filter((box) => {
        if (box.arrayRoot)
            return false;
        const path = aggregatePath(box);
        return path.length === prefix.length + 1 && pathStartsWith(path, prefix);
    });
    for (const member of directMembers) {
        const path = aggregatePath(member);
        const memberName = path[path.length - 1] || member.name;
        if (member.aggregateKind) {
            const child = makeAggregateBox(member, descendants, {
                ...opts,
                displayName: memberName,
            });
            membersNode.appendChild(child);
            continue;
        }
        if (member.typeInfo?.kind === "array") {
            const arrayBoxes = [
                member,
                ...descendants.filter((box) => box.arrayRoot === member.name),
            ];
            const group = groupStateObjects(arrayBoxes).find((item) => item.kind === "array");
            if (group) {
                membersNode.appendChild(makeArrayBox(group, {
                    editable: opts.editable,
                    deletable: opts.deletable,
                    displayName: memberName,
                }));
                continue;
            }
        }
        appendScalarStateBox(membersNode, member, {
            ...opts,
            displayName: memberName,
        });
    }
    if (opts.deletable && root.allowDelete) {
        const del = el('<button class="delete" title="delete">×</button>');
        node.appendChild(del);
        del.addEventListener("click", () => node.remove());
    }
    return node;
}
function appendStateObjects(container, boxes, opts = {}) {
    const { editable = false, deletable = editable, allowNameEdit = null, allowTypeEdit = null, } = opts;
    const source = Array.isArray(boxes) ? boxes : [];
    const originalIndex = new Map(source.map((box, index) => [box, index]));
    const aggregateRoots = source.filter((box) => !!box.aggregateKind && !box.aggregateRoot);
    const aggregateRootNames = new Set(aggregateRoots.map((box) => box.name));
    const ordinaryBoxes = source.filter((box) => !aggregateRoots.includes(box) &&
        !(box.aggregateRoot && aggregateRootNames.has(box.aggregateRoot)));
    const renderItems = aggregateRoots.map((root) => ({
        kind: "aggregate",
        root,
        descendants: source.filter((box) => box.aggregateRoot === root.name),
        index: originalIndex.get(root) ?? 0,
    }));
    for (const object of groupStateObjects(ordinaryBoxes)) {
        const index = object.kind === "scalar"
            ? (originalIndex.get(object.box) ?? object.index)
            : Math.min(...object.entries.map((entry) => originalIndex.get(entry.box) ?? entry.index));
        renderItems.push({ kind: "ordinary", object, index });
    }
    renderItems.sort((left, right) => left.index - right.index);
    for (const item of renderItems) {
        if (item.kind === "aggregate") {
            container.appendChild(makeAggregateBox(item.root, item.descendants, {
                editable,
                deletable,
                allowNameEdit,
                allowTypeEdit,
            }));
            continue;
        }
        if (item.object.kind === "scalar") {
            appendScalarStateBox(container, item.object.box, {
                editable,
                deletable,
                allowNameEdit,
                allowTypeEdit,
            });
        }
        else {
            container.appendChild(makeArrayBox(item.object, {
                editable,
                deletable,
            }));
        }
    }
}
function readArrayBoxState(root) {
    const shape = normalizeArrayDims(String(root.dataset.arrayShape || "")
        .split(",")
        .filter(Boolean)
        .map((value) => Number(value)));
    const rootName = String(root.dataset.arrayRoot || "").trim();
    const values = [...root.querySelectorAll(".array-col-value")];
    return values.map((valueNode) => {
        const valueEl = valueNode;
        const typeText = String(valueEl.dataset.arrayType || root.dataset.arrayElementType || "int").trim();
        const valText = txt(valueEl);
        const placeholderEmpty = valueEl.classList.contains("placeholder") && valText === "";
        const storedRawValue = valueEl.dataset.rawValue;
        const fallbackRawValue = placeholderEmpty ? "" : valText;
        const rawValue = storedRawValue !== undefined ? storedRawValue : fallbackRawValue;
        const value = normalizeZeroDisplay(rawValue);
        const indices = String(valueEl.dataset.arrayIndices || "")
            .split(",")
            .filter(Boolean)
            .map((num) => Math.floor(Number(num)))
            .filter((num) => Number.isFinite(num) && num >= 0);
        const fallbackName = rootName && indices.length ? arrayElementName(rootName, indices) : "";
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
            allowDelete: root.dataset.allowDelete === "true" ||
                (root.querySelector(".delete") != null),
        };
    });
}
function serializeWorkspace(target) {
    if (!target)
        return null;
    let ws = null;
    if (typeof target === "string") {
        ws = document.getElementById(target);
        if (!ws)
            return null;
    }
    else {
        ws = target;
    }
    if (!ws)
        return null;
    const wsEl = ws;
    let nodes = [...ws.querySelectorAll(".vbox, .arraybox")];
    if (!nodes.length && wsEl.dataset.inline === "true") {
        const key = wsEl.dataset.workspaceKey || "";
        if (key) {
            nodes = [
                ...document.querySelectorAll(`.vbox[data-workspace="${key}"], .arraybox[data-workspace="${key}"]`),
            ];
        }
    }
    const out = [];
    nodes.forEach((node) => {
        const el = node;
        if (el.classList.contains("arraybox")) {
            out.push(...readArrayBoxState(el));
            return;
        }
        out.push(readBoxState(el));
    });
    return out;
}
function restoreWorkspace(state, defaults, opts = {}) {
    const { editable = true, deletable = editable, allowNameEdit = null, allowTypeEdit = null, } = opts;
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
function parseStyledText(text) {
    const map = {
        n: "name",
        t: "type",
        v: "value",
        a: "addr",
        c: "code",
        b: "btn",
        i: "italic",
    };
    const raw = text;
    const out = [];
    let i = 0;
    while (i < raw.length) {
        const idx = raw.indexOf("$", i);
        if (idx < 0) {
            out.push(raw.slice(i));
            break;
        }
        if (idx > i)
            out.push(raw.slice(i, idx));
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
function renderParts(panel, parts) {
    if (!panel)
        return;
    clearNode(panel);
    const list = Array.isArray(parts) ? parts : [parts];
    const appendText = (value) => {
        const text = String(value);
        const chunks = text.split("\n");
        chunks.forEach((chunk, idx) => {
            if (idx > 0)
                panel.appendChild(document.createElement("br"));
            if (chunk)
                panel.appendChild(document.createTextNode(chunk));
        });
    };
    const appendToken = (role, text) => {
        const chunks = text.split("\n");
        chunks.forEach((chunk, idx) => {
            if (idx > 0)
                panel.appendChild(document.createElement("br"));
            let node;
            if (role === "btn") {
                node = document.createElement("span");
                node.className = "btn-ref";
                node.dataset.btnRef = chunk;
            }
            else if (role === "italic") {
                node = document.createElement("em");
                node.className = "tok-italic";
            }
            else {
                node = document.createElement("code");
                node.className = role ? `tok-${role}` : "";
            }
            node.textContent = chunk;
            panel.appendChild(node);
        });
    };
    const appendPart = (part) => {
        if (part === null || part === undefined)
            return;
        if (typeof part === "string" || typeof part === "number") {
            const parsed = parseStyledText(String(part));
            parsed.forEach((segment) => {
                if (typeof segment === "object" && segment.kind === "tok") {
                    const role = String(segment.role || "").trim();
                    appendToken(role, segment.text);
                }
                else {
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
function bindBtnRefPulse(root = document) {
    if (!root || root.dataset?.btnRefPulseBound === "1")
        return;
    const host = root;
    if (host.dataset)
        host.dataset.btnRefPulseBound = "1";
    const clear = () => {
        document
            .querySelectorAll("button.btn-ref-hover")
            .forEach((btn) => btn.classList.remove("btn-ref-hover"));
    };
    host.addEventListener("mouseover", (event) => {
        const target = event?.target?.closest?.(".btn-ref");
        if (!target)
            return;
        const label = (target.dataset.btnRef || target.textContent || "").trim();
        if (!label)
            return;
        document.querySelectorAll("button").forEach((btn) => {
            if (btn.textContent?.trim() === label)
                btn.classList.add("btn-ref-hover");
        });
    }, true);
    host.addEventListener("mouseout", (event) => {
        const target = event?.target?.closest?.(".btn-ref");
        if (!target)
            return;
        clear();
    }, true);
    const observer = new MutationObserver(() => clear());
    observer.observe(document.body, { childList: true, subtree: true });
}
function setPartsContent(panel, parts) {
    if (!panel)
        return;
    if (!parts || (Array.isArray(parts) && parts.length === 0)) {
        clearNode(panel);
        panel.classList.add("hidden");
        return;
    }
    panel.classList.remove("hidden");
    renderParts(panel, parts);
}
function stepperButtons(root, dir) {
    const list = [];
    if (!root)
        return list;
    root.querySelectorAll(`[data-stepper="${dir}"]`).forEach((btn) => {
        if (btn instanceof HTMLButtonElement && !list.includes(btn))
            list.push(btn);
    });
    return list;
}
function createStepper({ root, prevButtons = null, nextButtons = null, lines = [], previousPage = getPreviousNavHref(), nextPage = null, getBoundary, setBoundary, onBeforeChange, onAfterChange, isStepLocked, getStepBadge, getNextLabel, getNextBoundary, getPrevBoundary, isAtEnd, startLabel, endLabel, allowSameBoundary = false, } = {}) {
    const boundButtons = new WeakSet();
    const getPrevButtons = () => prevButtons || stepperButtons(root, "prev");
    const getNextButtons = () => nextButtons || stepperButtons(root, "next");
    const total = Array.isArray(lines)
        ? lines.length
        : Math.max(0, Number(lines) || 0);
    const resolvedStartLabel = (() => {
        if (startLabel)
            return startLabel;
        const label = getNavLabelForHref(previousPage);
        return label ? `Prev: ${label}` : "Previous Program";
    })();
    function clearPulse() {
        getNextButtons().forEach((btn) => btn.classList.remove("pulse-success"));
    }
    function boundary() {
        return typeof getBoundary === "function" ? getBoundary() : 0;
    }
    function setBoundaryValue(value) {
        if (typeof setBoundary === "function")
            setBoundary(value);
    }
    function atEnd(at) {
        return typeof isAtEnd === "function" ? !!isAtEnd(at, total) : at === total;
    }
    function locked(at) {
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
            const customLabel = typeof getNextLabel === "function"
                ? getNextLabel(current, total, atEndNow)
                : "";
            const badge = !atEndNow && typeof getStepBadge === "function"
                ? getStepBadge(current + 1)
                : "";
            const badgeTag = badge === "note" ? "🔧" : badge === "check" ? "✅" : "";
            const labelPrefix = badgeTag ? `${badgeTag} ` : "";
            const adjustLabelForBadge = (label) => {
                if (badge !== "note")
                    return label;
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
            }
            else {
                const label = customLabel || `Run line ${current + 1}`;
                const adjustedLabel = adjustLabelForBadge(label);
                const isEndPreview = !!customLabel && !!endLabel && customLabel === endLabel;
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
    function withSidebarParam(url) {
        if (!url)
            return url;
        const [base, hash = ""] = url.split("#");
        const [path, query = ""] = base.split("?");
        const params = new URLSearchParams(query);
        params.set("sidebar", sidebarParamValue());
        const nextQuery = params.toString();
        const hashPart = hash ? `#${hash}` : "";
        return nextQuery ? `${path}?${nextQuery}${hashPart}` : `${path}${hashPart}`;
    }
    function goTo(target) {
        const current = boundary();
        const clamped = Math.max(0, Math.min(total, target));
        if (clamped === current) {
            if (!allowSameBoundary)
                return;
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
            if (boundButtons.has(btn))
                return;
            boundButtons.add(btn);
            btn.addEventListener("click", () => {
                if (boundary() === 0) {
                    if (!btn.disabled && previousPage) {
                        const previousUrl = withSidebarParam(previousPage);
                        if (previousUrl)
                            window.location.href = previousUrl;
                    }
                    return;
                }
                clearPulse();
                const current = boundary();
                const target = typeof getPrevBoundary === "function"
                    ? getPrevBoundary(current, total)
                    : current - 1;
                goTo(target);
            });
        });
        getNextButtons().forEach((btn) => {
            if (boundButtons.has(btn))
                return;
            boundButtons.add(btn);
            btn.addEventListener("click", () => {
                const current = boundary();
                clearPulse();
                if (atEnd(current)) {
                    if (!btn.disabled && nextPage) {
                        const nextUrl = withSidebarParam(nextPage);
                        if (nextUrl)
                            window.location.href = nextUrl;
                    }
                    return;
                }
                if (locked(current))
                    return;
                const target = typeof getNextBoundary === "function"
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
function syncPlaceholderMutedState(node) {
    if (!node || !node.classList?.contains("placeholder"))
        return;
    if (txt(node) === "")
        node.classList.add("muted");
    else
        node.classList.remove("muted");
}
function initEditableFieldHandlers() {
    if (document.body.dataset.editableFieldHandlersReady === "1")
        return;
    document.body.dataset.editableFieldHandlersReady = "1";
    document.addEventListener("focusin", (event) => {
        const target = event.target;
        if (!target)
            return;
        disableAutoText(target);
        if (target.classList?.contains("code-editable") &&
            target.classList.contains("placeholder")) {
            const placeholder = target.dataset?.placeholder || "";
            if (txt(target) === placeholder) {
                target.textContent = "";
                target.classList.remove("placeholder", "muted");
            }
        }
        syncPlaceholderMutedState(target);
    });
    document.addEventListener("input", (event) => {
        const target = event.target;
        syncPlaceholderMutedState(target);
    });
    document.addEventListener("keydown", (event) => {
        if (event.key !== "Enter")
            return;
        const target = event.target;
        if (!target?.isContentEditable)
            return;
        if (target.classList?.contains("value") ||
            target.classList?.contains("type") ||
            target.classList?.contains("name-text")) {
            event.preventDefault();
            target.blur();
        }
    });
    document.addEventListener("focusout", (event) => {
        const target = event.target;
        if (!target)
            return;
        if (target.classList?.contains("code-editable")) {
            const placeholder = target.dataset?.placeholder || "";
            if (!txt(target)) {
                target.textContent = placeholder;
                if (placeholder)
                    target.classList.add("placeholder", "muted");
            }
        }
        if (target.classList?.contains("placeholder") && txt(target) === "") {
            target.textContent = "";
            target.classList.add("muted");
        }
    });
}
function isTextInputActive(el) {
    if (!el)
        return false;
    if (el.isContentEditable)
        return true;
    const tag = el.tagName;
    return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT";
}
document.addEventListener("keydown", (e) => {
    if (e.key !== "ArrowLeft" && e.key !== "ArrowRight")
        return;
    if (isTextInputActive(document.activeElement) ||
        isTextInputActive(e.target))
        return;
    const selector = e.key === "ArrowLeft"
        ? 'button[data-stepper="prev"]'
        : 'button[data-stepper="next"]';
    const btn = [...document.querySelectorAll(selector)].find((node) => node instanceof HTMLButtonElement &&
        !node.disabled &&
        node.dataset.stepperStart !== "true" &&
        node.dataset.stepperEnd !== "true");
    if (!btn || btn.disabled)
        return;
    e.preventDefault();
    btn.click();
});
const customScrollbarState = new WeakMap();
function initCustomPanelScrollbars() {
    const states = new Set();
    const selectors = [
        ".panel-scroll > .panel-body",
        ".state-panel.state-panel-scrollable .state-panel-scroll-body",
        ".sandbox-expr-row [data-role=\"sandbox-expr-result\"]",
        ".expr-answer-result",
    ];
    const updateState = (state) => {
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
        }
        else {
            const visibleY = host.clientHeight;
            const trackHeight = Math.max(0, visibleY - (hasX ? 14 : 0));
            trackY.style.top = `${Math.round(top)}px`;
            trackY.style.height = `${Math.round(trackHeight)}px`;
            trackY.classList.remove("hidden");
            const thumbHeight = Math.max(28, Math.round((visibleY * visibleY) / host.scrollHeight));
            const maxTravel = Math.max(0, trackHeight - thumbHeight);
            const ratio = scrollableY > 0 ? host.scrollTop / scrollableY : 0;
            const thumbTop = Math.max(0, Math.min(maxTravel, Math.round(maxTravel * ratio)));
            thumbY.style.height = `${thumbHeight}px`;
            thumbY.style.transform = `translateY(${thumbTop}px)`;
        }
        if (!hasX) {
            trackX.classList.add("hidden");
        }
        else {
            const visibleX = host.clientWidth;
            const trackWidth = Math.max(0, visibleX - (hasY ? 14 : 0));
            trackX.style.left = `${Math.round(left)}px`;
            trackX.style.top = `${Math.round(top + host.clientHeight - 12)}px`;
            trackX.style.width = `${Math.round(trackWidth)}px`;
            trackX.classList.remove("hidden");
            const thumbWidth = Math.max(28, Math.round((visibleX * visibleX) / host.scrollWidth));
            const maxTravel = Math.max(0, trackWidth - thumbWidth);
            const ratio = scrollableX > 0 ? host.scrollLeft / scrollableX : 0;
            const thumbLeft = Math.max(0, Math.min(maxTravel, Math.round(maxTravel * ratio)));
            thumbX.style.width = `${thumbWidth}px`;
            thumbX.style.transform = `translateX(${thumbLeft}px)`;
        }
    };
    const scheduleUpdate = (state) => {
        if (state.rafId != null)
            return;
        state.rafId = requestAnimationFrame(() => {
            state.rafId = null;
            updateState(state);
        });
    };
    const bindHost = (host) => {
        if (customScrollbarState.has(host))
            return;
        const panel = host.closest(".panel, .state-panel");
        if (!panel)
            return;
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
        const state = {
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
        const onMoveY = (event) => {
            if (!draggingY)
                return;
            const scrollable = Math.max(0, host.scrollHeight - host.clientHeight);
            if (scrollable <= 0)
                return;
            const thumbHeight = thumbY.getBoundingClientRect().height;
            const maxTravel = Math.max(1, trackY.getBoundingClientRect().height - thumbHeight);
            const deltaY = event.clientY - startY;
            const scrollDelta = (deltaY / maxTravel) * scrollable;
            host.scrollTop = Math.max(0, Math.min(scrollable, startScrollTop + scrollDelta));
            scheduleUpdate(state);
        };
        const stopDraggingY = () => {
            if (!draggingY)
                return;
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
            if (event.target === thumbY)
                return;
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
        const onMoveX = (event) => {
            if (!draggingX)
                return;
            const scrollable = Math.max(0, host.scrollWidth - host.clientWidth);
            if (scrollable <= 0)
                return;
            const thumbWidth = thumbX.getBoundingClientRect().width;
            const maxTravel = Math.max(1, trackX.getBoundingClientRect().width - thumbWidth);
            const deltaX = event.clientX - startX;
            const scrollDelta = (deltaX / maxTravel) * scrollable;
            host.scrollLeft = Math.max(0, Math.min(scrollable, startScrollLeft + scrollDelta));
            scheduleUpdate(state);
        };
        const stopDraggingX = () => {
            if (!draggingX)
                return;
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
            if (event.target === thumbX)
                return;
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
        const seen = new Set();
        selectors.forEach((selector) => {
            document.querySelectorAll(selector).forEach((node) => {
                if (!(node instanceof HTMLElement) || seen.has(node))
                    return;
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
    if (document.body.dataset.sidebarStateApplied === "1")
        return;
    const params = new URLSearchParams(window.location.search);
    const state = params.get("sidebar");
    if (state === "0")
        document.body.classList.add("sidebar-collapsed");
    if (state === "1")
        document.body.classList.remove("sidebar-collapsed");
    if (state == null) {
        const prefersCollapsed = isMobileViewport();
        if (prefersCollapsed)
            document.body.classList.add("sidebar-collapsed");
    }
    document.body.dataset.sidebarStateApplied = "1";
}
onDomReady(() => {
    initEditableFieldHandlers();
    applyAutoTextDefaults(document);
    initCustomPanelScrollbars();
    ensureBaseLayout();
});
function flashStatus(el) {
    const node = el;
    if (!node)
        return;
    node.classList.remove("status-flash");
    // force reflow to restart animation
    void node.offsetWidth;
    node.classList.add("status-flash");
}
export { clearNode, buildNav, createStepper, disableBoxEditing, ensureBaseLayout, ensurePanelizedMain, flashStatus, getNavLabelForHref, getPreviousNavHref, isMobileViewport, bindBtnRefPulse, makeAnswerBox, queryElement, queryRole, readBoxState, removeBoxDeleteButtons, renderCodePane, renderParts, resolveActiveNavItem, restoreWorkspace, serializeWorkspace, setPartsContent, syncDocumentTitleFromNav, vbox, applyOtherNames, appendStateObjects, findArrayObjectBoxesForResult, };
