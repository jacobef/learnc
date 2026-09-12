import { normalizeZeroDisplay } from "./shared-core-utils.js";
import { disableAutoText, el, txt } from "./shared-dom-utils.js";
import { attachTypeHelp } from "./shared-type-help.js";
export function vbox({ address = "—", type = "int", value = "", name = "", editable = false, allowNameEdit = false, allowTypeEdit = false, showDoubleExact = false, displayValue = null, exactValue = null, typeInfo = null, aliases = [], stateName = null, } = {}) {
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
export function disableBoxEditing(root) {
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
export function removeBoxDeleteButtons(root) {
    const scope = root || document;
    scope
        .querySelectorAll(".vbox .delete, .arraybox .delete, .aggregatebox .delete")
        .forEach((btn) => btn.remove());
}
export function readBoxState(root) {
    const el = root;
    const names = [...root.querySelectorAll(".name-text")]
        .map((el) => txt(el))
        .filter(Boolean);
    const valEl = root.querySelector(".value");
    const valText = txt(valEl);
    const typeText = txt(root.querySelector(".type"));
    const storedRawValue = valEl instanceof HTMLElement ? valEl.dataset.rawValue : undefined;
    const rawValue = storedRawValue ?? valText;
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
export function applyOtherNames(root, opts = {}) {
    if (!root)
        return;
    const { onToggle = null, shownAddrs = null, sourceBoxes = null, cleanupShownAddrs = true, } = opts;
    const renderedBoxes = [...root.querySelectorAll(".vbox")].map((node) => ({ node, box: readBoxState(node) }));
    const boxes = renderedBoxes.map(({ box }) => box);
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
    renderedBoxes.forEach(({ node, box }) => {
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
export function makeAnswerBox({ name = "", type = "", value = "", address = null, editable = true, deletable = editable, allowNameEdit = null, allowTypeEdit = null, showDoubleExact = null, displayValue = null, exactValue = null, typeInfo = null, aliases = [], stateName = null, } = {}) {
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
export function findArrayObjectBoxesForResult(result, state) {
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
export function appendStateObjects(container, boxes, opts = {}) {
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
    return values.map((valueEl) => {
        const typeText = String(valueEl.dataset.arrayType || root.dataset.arrayElementType || "int").trim();
        const valText = txt(valueEl);
        const rawValue = valueEl.dataset.rawValue ?? valText;
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
export function serializeWorkspace(target) {
    if (!target)
        return null;
    const nodes = target.querySelectorAll(".vbox, .arraybox");
    const out = [];
    nodes.forEach((node) => {
        if (node.classList.contains("arraybox")) {
            out.push(...readArrayBoxState(node));
            return;
        }
        out.push(readBoxState(node));
    });
    return out;
}
export function restoreWorkspace(state, defaults, opts = {}) {
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
export function renderStatePanel(title, boxes, { emptyMessage = "(no variables)", controls } = {}) {
    const panel = document.createElement("div");
    panel.className = "state-panel state-panel-scrollable";
    const heading = document.createElement("h3");
    heading.className = "panel-title state-heading";
    heading.textContent = title;
    panel.appendChild(heading);
    if (controls) {
        const controlsWrap = document.createElement("div");
        controlsWrap.className = "state-panel-controls";
        controlsWrap.appendChild(controls);
        panel.appendChild(controlsWrap);
    }
    const grid = document.createElement("div");
    grid.className = "grid";
    if (boxes?.length) {
        appendStateObjects(grid, boxes, { editable: false, deletable: false });
    }
    else {
        const message = document.createElement("div");
        message.className = "muted state-empty-message";
        message.textContent = emptyMessage;
        grid.appendChild(message);
    }
    const body = document.createElement("div");
    body.className = "state-panel-scroll-body";
    body.appendChild(grid);
    panel.appendChild(body);
    return panel;
}
