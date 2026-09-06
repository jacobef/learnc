import type { CTypeHelpNode, CTypeInfo } from "./shared-core-utils.js";

let nextTypeHelpId = 0;

function appendTypeHelpConnector(
  parent: HTMLElement,
  text: string,
  punctuation = false,
): void {
  const connector = document.createElement("span");
  connector.className = punctuation
    ? "type-help-connector type-help-punctuation"
    : "type-help-connector";
  connector.textContent = punctuation ? text : ` ${text} `;
  parent.append(connector);
}

function pluralizeTypeHelpLabel(node: CTypeHelpNode): string {
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

function typeHelpIndefiniteArticle(node: CTypeHelpNode): "a" | "an" | null {
  if (node.kind === "type" && node.label === "void") return null;
  if (/^union\b/i.test(node.label) || node.label === "_Bool") return "a";
  const spokenLabel = node.label
    .replace(/^_Atomic\b/i, "atomic")
    .replace(/^_+/, "");
  return /^[aeiou]/i.test(spokenLabel) ? "an" : "a";
}

function connectorWithArticle(prefix: string, node: CTypeHelpNode): string {
  const article = typeHelpIndefiniteArticle(node);
  return article ? `${prefix} ${article}`.trim() : prefix;
}

type TypeHelpKindCounts = Map<CTypeHelpNode["kind"], number>;

function nextTypeHelpShadeClass(
  node: CTypeHelpNode,
  kindCounts: TypeHelpKindCounts,
): string {
  if (node.kind !== "pointer" && node.kind !== "array" && node.kind !== "function") {
    return "";
  }

  const occurrence = kindCounts.get(node.kind) ?? 0;
  kindCounts.set(node.kind, occurrence + 1);
  return `type-help-shade-${occurrence % 3}`;
}

function typeHelpTreeRelation(
  parent: CTypeHelpNode,
  relation: string,
): string {
  if (
    (parent.kind === "pointer" && relation === "to")
    || (parent.kind === "array" && relation === "of")
  ) {
    return "";
  }
  const parameter = /^parameter\s+(\d+)$/.exec(relation);
  return parameter ? `arg ${parameter[1]}` : relation;
}

function typeHelpTreeConnector(relation: string): HTMLElement {
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

function renderTypeHelpTree(node: CTypeHelpNode): HTMLElement {
  const element = document.createElement("div");
  element.className = "type-help-tree-node";

  const row = document.createElement("div");
  row.className = "type-help-tree-node-row";

  const label = document.createElement("div");
  label.className = `type-help-tree-label type-help-tree-label-${node.kind}`;
  const functionTakesNoArguments =
    node.kind === "function" && node.label === "function taking no arguments";
  label.textContent = functionTakesNoArguments ? "function" : node.label;
  if (node.typeName) label.classList.add("type-help-type");
  row.append(label);

  if (node.children.length === 1 && !functionTakesNoArguments) {
    const child = node.children[0];
    row.append(
      typeHelpTreeConnector(typeHelpTreeRelation(node, child.relation)),
      renderTypeHelpTree(child.node),
    );
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
      branch.append(
        typeHelpTreeConnector(typeHelpTreeRelation(node, child.relation)),
        renderTypeHelpTree(child.node),
      );
      children.append(branch);
    }
    element.append(children);
  }

  return element;
}

function renderExpandedTypeHelp(
  node: CTypeHelpNode,
  pluralHead = false,
  kindCounts: TypeHelpKindCounts = new Map(),
): HTMLElement {
  const element = document.createElement("span");
  element.className = `type-help-phrase type-help-phrase-${node.kind}`;
  const shadeClass = nextTypeHelpShadeClass(node, kindCounts);
  if (shadeClass) element.classList.add(shadeClass);

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
  } else {
    label.textContent = displayedLabel;
  }
  if (node.typeName) label.classList.add("type-help-type");
  element.append(label);

  if (node.kind === "array" && node.children.length === 1) {
    element.append(
      " ",
      ...(node.label === "array of unknown length" ? ["containing "] : []),
      renderExpandedTypeHelp(node.children[0].node, true, kindCounts),
    );
  } else if (node.kind === "function") {
    const parameters = node.children.filter((child) =>
      child.relation.startsWith("parameter ")
    );
    const returnType = node.children.find((child) => child.relation === "returns");
    if (parameters.length) {
      parameters.forEach((parameter, index) => {
        const connector =
          index === 0
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
      const labelAlreadyDescribesArguments =
        node.label.includes("argument") || node.label.includes("taking");
      appendTypeHelpConnector(
        element,
        connectorWithArticle(
          parameters.length || labelAlreadyDescribesArguments
            ? "and returning"
            : "returning",
          returnType.node,
        ),
      );
      element.append(renderExpandedTypeHelp(returnType.node, false, kindCounts));
    }
  } else {
    for (const child of node.children) {
      const isDistributedPointer =
        pluralHead && node.kind === "pointer" && child.relation === "to";
      if (isDistributedPointer) {
        appendTypeHelpConnector(element, ",", true);
      }
      appendTypeHelpConnector(
        element,
        connectorWithArticle(
          isDistributedPointer ? "each pointing to" : child.relation,
          child.node,
        ),
      );
      element.append(renderExpandedTypeHelp(child.node, false, kindCounts));
    }
  }

  return element;
}

function conciseTypeHelpToken(
  text: string,
  typeName = false,
): HTMLElement {
  const token = document.createElement("span");
  token.className = "type-help-concise-token";
  token.textContent = text;
  if (typeName) token.classList.add("type-help-type");
  return token;
}

function renderConciseTypeHelp(
  node: CTypeHelpNode,
  kindCounts: TypeHelpKindCounts = new Map(),
): HTMLElement {
  const element = document.createElement("span");
  element.className = `type-help-phrase type-help-phrase-${node.kind}`;
  const shadeClass = nextTypeHelpShadeClass(node, kindCounts);
  if (shadeClass) element.classList.add(shadeClass);

  if (node.kind === "type") {
    element.append(conciseTypeHelpToken(node.label, !!node.typeName));
    return element;
  }

  if (node.kind === "pointer") {
    let pointerCount = 1;
    let tail = node;
    while (
      tail.children.length === 1
      && tail.children[0].node.kind === "pointer"
    ) {
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
    element.append(
      conciseTypeHelpToken(length === "unknown length" ? "[]" : `[${length}]`),
    );
    for (const child of node.children) {
      element.append(renderConciseTypeHelp(child.node, kindCounts));
    }
    return element;
  }

  const parameters = node.children.filter((child) =>
    child.relation.startsWith("parameter ")
  );
  const returnType = node.children.find((child) => child.relation === "returns");
  const openingParen = conciseTypeHelpToken("(");
  openingParen.classList.add("type-help-concise-paren");
  element.append(openingParen);
  parameters.forEach((parameter, index) => {
    if (index) element.append(conciseTypeHelpToken(","));
    element.append(renderConciseTypeHelp(parameter.node, kindCounts));
  });
  if (node.label.includes("variadic")) {
    if (parameters.length) element.append(conciseTypeHelpToken(","));
    element.append(conciseTypeHelpToken("…"));
  } else if (node.label.includes("unspecified")) {
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

type TypeHelpViewMode = "english" | "concise" | "tree";

export function attachTypeHelp(
  root: HTMLElement,
  typeSelector: string,
  typeInfo: CTypeInfo | null | undefined,
): void {
  const help = String(typeInfo?.help ?? "").trim();
  if (!help) return;
  const typeEl = root.querySelector(typeSelector) as HTMLElement | null;
  if (!typeEl || typeEl.closest(".type-value-row")) return;
  const precedingElement = typeEl.previousElementSibling;
  const typeLabel =
    precedingElement instanceof HTMLElement && precedingElement.matches(".lbl")
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
  const modeButtons: Array<{
    mode: TypeHelpViewMode;
    button: HTMLButtonElement;
  }> = [];
  let explanation: HTMLElement | null = null;
  let tree: HTMLElement | null = null;
  if (typeInfo?.helpTree) {
    const modeSwitch = document.createElement("div");
    modeSwitch.className = "type-help-mode-switch";
    modeSwitch.setAttribute("role", "group");
    modeSwitch.setAttribute("aria-label", "Type explanation view");
    const modes: Array<{ mode: TypeHelpViewMode; label: string }> = [
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
  } else {
    tooltip.textContent = help;
  }
  const supportsPopover = typeof tooltip.showPopover === "function";
  if (supportsPopover) tooltip.setAttribute("popover", "manual");

  const placeTooltip = () => {
    const buttonRect = button.getBoundingClientRect();
    const typeRect = typeEl.getBoundingClientRect();
    const tooltipRect = tooltip.getBoundingClientRect();
    const viewportPadding = 12;
    const gap = 7;
    const maxLeft = Math.max(
      viewportPadding,
      window.innerWidth - tooltipRect.width - viewportPadding,
    );
    const left = Math.min(
      maxLeft,
      Math.max(viewportPadding, buttonRect.right - tooltipRect.width),
    );
    const anchorTop = Math.min(buttonRect.top, typeRect.top);
    const anchorBottom = Math.max(buttonRect.bottom, typeRect.bottom);
    let top = anchorBottom + gap;
    if (
      top + tooltipRect.height > window.innerHeight - viewportPadding &&
      anchorTop - tooltipRect.height - gap >= viewportPadding
    ) {
      top = anchorTop - tooltipRect.height - gap;
    }
    top = Math.min(
      Math.max(viewportPadding, top),
      Math.max(viewportPadding, window.innerHeight - tooltipRect.height - viewportPadding),
    );
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
  const selectTypeHelpMode = (selectedMode: TypeHelpViewMode) => {
    if (!explanation || !tree || !typeInfo?.helpTree) return;
    for (const { mode, button: modeButton } of modeButtons) {
      modeButton.setAttribute("aria-pressed", String(mode === selectedMode));
    }
    const showTree = selectedMode === "tree";
    explanation.hidden = showTree;
    tree.hidden = !showTree;
    if (!showTree) {
      const concise = selectedMode === "concise";
      explanation.classList.toggle("is-concise", concise);
      explanation.replaceChildren(
        concise
          ? renderConciseTypeHelp(typeInfo.helpTree)
          : renderExpandedTypeHelp(typeInfo.helpTree),
      );
    }
    placeTooltip();
  };
  for (const { mode, button: modeButton } of modeButtons) {
    modeButton.addEventListener("click", () => selectTypeHelpMode(mode));
  }
  const showTooltip = () => {
    if (supportsPopover) {
      if (!tooltip.matches(":popover-open")) tooltip.showPopover();
    } else {
      tooltip.classList.add("is-open");
    }
    button.setAttribute("aria-expanded", "true");
    placeTooltip();
  };
  const closeTooltip = () => {
    if (supportsPopover) {
      if (tooltip.matches(":popover-open")) tooltip.hidePopover();
    } else {
      tooltip.classList.remove("is-open");
    }
    button.setAttribute("aria-expanded", "false");
    hoverBridge.classList.remove("is-active");
  };
  const hideTooltip = () => {
    if (
      control.matches(":hover")
      || tooltip.matches(":hover")
      || control.contains(document.activeElement)
    ) {
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
    } else {
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
  } else {
    row.appendChild(control);
  }
}
