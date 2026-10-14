import { normalizeZeroDisplay, sameCType } from "./shared-core-utils.js";
import { boxValueMatchesSpec } from "./shared-c-value-semantics.js";
/** Give inaccessible objects distinct display names without changing source-level names. */
export function memoryDisplayState(boxes) {
    const roots = new Map();
    for (const box of boxes) {
        if (box.objectId !== undefined && !roots.has(box.objectId)) {
            roots.set(box.objectId, box.aggregateRoot || box.arrayRoot || box.name);
        }
    }
    return boxes.map(box => {
        if (box.nameAccessible !== false || box.objectId === undefined || box.name.includes(" (out of scope · "))
            return box;
        const root = roots.get(box.objectId);
        const label = `${root} (out of scope · ${box.scope || "object"} #${box.objectId})`;
        const rename = (name) => name.startsWith(root) ? label + name.slice(root.length) : name;
        return {
            ...box,
            name: rename(box.name),
            arrayRoot: box.arrayRoot ? rename(box.arrayRoot) : box.arrayRoot,
            aggregateRoot: box.aggregateRoot ? rename(box.aggregateRoot) : box.aggregateRoot,
        };
    });
}
// Whether a type name like "int *[3]" or "int (*[2])[3]" is an array type, and
// not, like "int (*)[3]", a pointer to one. The declarator in the innermost
// parentheses, where the name would go, decides.
function isArrayTypeName(type) {
    const groups = [...type.matchAll(/\(([\s*[\]\d]*)\)/g)];
    const declarator = groups.length ? groups[groups.length - 1][1] : type;
    return /\[\s*\d+\s*\]\s*$/.test(declarator);
}
function isArrayRootStateBox(box) {
    if (box.arrayRoot)
        return false;
    const shape = Array.isArray(box.arrayShape) ? box.arrayShape : [];
    const indices = Array.isArray(box.arrayIndices) ? box.arrayIndices : [];
    if (shape.length > 0 && indices.length === 0)
        return true;
    if (box.typeInfo)
        return box.typeInfo.kind === "array";
    return isArrayTypeName(String(box.type || ""));
}
export function stateWithArrayRoots(boxes) {
    const result = boxes.slice();
    const rootNames = new Set(result
        .filter((box) => isArrayRootStateBox(box))
        .map((box) => String(box.name || "").trim())
        .filter(Boolean));
    const firstElementByRoot = new Map();
    for (const box of result) {
        const rootName = String(box.arrayRoot || "").trim();
        if (rootName && !firstElementByRoot.has(rootName)) {
            firstElementByRoot.set(rootName, box);
        }
    }
    for (const [rootName, first] of firstElementByRoot) {
        if (rootNames.has(rootName))
            continue;
        const shape = Array.isArray(first.arrayShape)
            ? first.arrayShape
                .map((value) => Math.floor(Number(value)))
                .filter((value) => Number.isFinite(value) && value > 0)
            : [];
        if (!shape.length)
            continue;
        result.push({
            ...first,
            name: rootName,
            type: `${first.type}${shape.map((value) => `[${value}]`).join("")}`,
            value: "",
            rawValue: "",
            arrayRoot: null,
            arrayShape: shape,
            arrayIndices: [],
        });
    }
    return result;
}
// A struct or union itself, rather than one of its members. Like an array, it's
// just a container: its members hold the values.
function isAggregateStateBox(box) {
    return !!box.aggregateKind;
}
function comparableStateBoxes(boxes) {
    return boxes.filter((box) => !isArrayRootStateBox(box) && !isAggregateStateBox(box));
}
function comparableStateBoxKey(box) {
    const arrayRoot = String(box.arrayRoot || "").trim();
    if (arrayRoot) {
        const indices = Array.isArray(box.arrayIndices)
            ? box.arrayIndices.map((value) => Math.floor(Number(value))).join(",")
            : "";
        return `array:${arrayRoot}:${indices || box.name}`;
    }
    return `scalar:${box.name}`;
}
// Whole variables: arrays, structs, and unions count once, not once per part.
function visibleStateBoxes(boxes) {
    return stateWithArrayRoots(boxes).filter((box) => !box.arrayRoot && !box.aggregateRoot);
}
// An array element or a struct or union member, which holds part of a variable's value.
function isPartStateBox(box) {
    return !!box.arrayRoot || (!!box.aggregateRoot && !box.aggregateKind);
}
export function stateMatches(actual, expected) {
    const actualBoxes = comparableStateBoxes(actual);
    const actualByName = new Map(actualBoxes.map((box) => [comparableStateBoxKey(box), box]));
    if (actualByName.size !== actualBoxes.length)
        return false;
    const expectedByName = new Map(comparableStateBoxes(expected).map((box) => [
        comparableStateBoxKey(box),
        box,
    ]));
    if (actualByName.size !== expectedByName.size)
        return false;
    for (const [name, expectedBox] of expectedByName.entries()) {
        const actualBox = actualByName.get(name);
        if (!actualBox)
            return false;
        if (!sameCType(actualBox.type, expectedBox.type))
            return false;
        if (!boxValueMatchesSpec(actualBox, expectedBox).ok)
            return false;
    }
    return true;
}
// How a hint refers to part of a box, like "x's value". A nameless object, whose
// name is an internal key like "<allocated block #7>[1]", is described instead.
function namesPart(name, part) {
    const nameless = /^<([^#>]+?) #\d+>(.*)$/.exec(name);
    if (!nameless)
        return `$n{${name}}'s ${part}`;
    const [, kind, rest] = nameless;
    if (!rest)
        return `the ${part} of the ${kind}`;
    if (/^(?:\[\d+\])+$/.test(rest))
        return `the ${part} of element $v{${rest}} of the ${kind}`;
    const member = /^\.(\w+)$/.exec(rest)?.[1];
    return member
        ? `the ${part} of the ${kind}'s member $n{${member}}`
        : `the ${part} of $c{${rest}} in the ${kind}`;
}
// How a hint refers to a whole variable, or to a nameless object like a block
// from malloc.
function objectPhrase(name) {
    const nameless = /^<([^#>]+?) #\d+>$/.exec(name);
    return nameless ? `the ${nameless[1]}` : `the $n{${name}} variable`;
}
function capitalize(text) {
    return text.charAt(0).toUpperCase() + text.slice(1);
}
function formatNameList(names) {
    const tokens = names.map((name) => `$n{${name}}`);
    if (tokens.length === 1)
        return tokens[0] || "";
    if (tokens.length === 2)
        return `${tokens[0]} and ${tokens[1]}`;
    return `${tokens.slice(0, -1).join(", ")}, and ${tokens[tokens.length - 1]}`;
}
export function basicHintForBoxes(actual, expected, baseline, stage) {
    const visibleActual = visibleStateBoxes(actual);
    const visibleExpected = visibleStateBoxes(expected);
    const visibleBaseline = visibleStateBoxes(baseline);
    const actualCount = visibleActual.length;
    const expectedCount = visibleExpected.length;
    const nameOf = (box) => String(box?.name || "").trim();
    const typeOf = (box) => String(box?.type || "").trim();
    const expectedNames = visibleExpected.map(nameOf).filter(Boolean);
    const expectedNameSet = new Set(expectedNames);
    const actualNames = visibleActual.map(nameOf);
    const actualNameSet = new Set(actualNames.filter(Boolean));
    const missingExpectedNames = expectedNames.filter((name) => !actualNameSet.has(name));
    const baselineNames = new Set(visibleBaseline.map(nameOf).filter(Boolean));
    const removedName = missingExpectedNames.find((name) => baselineNames.has(name));
    if (removedName) {
        return {
            message: `This line shouldn't remove ${objectPhrase(removedName)}.`,
            kind: "removed",
            variable: removedName,
        };
    }
    const extraBaselineNames = actualNames.filter((name) => name && baselineNames.has(name) && !expectedNameSet.has(name));
    if (extraBaselineNames.length > 0) {
        const name = extraBaselineNames[0] || "";
        if (name) {
            return {
                message: `This line should remove ${objectPhrase(name)}.`,
                kind: "not-removed",
                variable: name,
            };
        }
    }
    if (actualCount < expectedCount) {
        const expectedName = missingExpectedNames[0] || expectedNames[0] || "";
        if (!expectedName) {
            return { message: "You need to add a new variable.", kind: "count" };
        }
        return {
            message: `You need to add ${objectPhrase(expectedName)}.`,
            kind: "count",
            variable: expectedName,
        };
    }
    if (actualCount === expectedCount && missingExpectedNames.length > 0) {
        const expectedNewNames = expectedNames.filter((name) => !baselineNames.has(name));
        if (expectedNewNames.length > 1) {
            return {
                message: `The new variables should be named ${formatNameList(expectedNewNames)}.`,
                kind: "name",
                variable: expectedNewNames[0],
            };
        }
        const expectedName = expectedNewNames[0] || missingExpectedNames[0] || "";
        if (!expectedName)
            return null;
        return {
            message: `The new variable should be named $n{${expectedName}}.`,
            kind: "name",
            variable: expectedName,
        };
    }
    if (actualCount > expectedCount) {
        const baselineCount = visibleBaseline.length;
        const expectedNew = Math.max(0, expectedCount - baselineCount);
        const actualNew = Math.max(0, actualCount - baselineCount);
        const extraCount = Math.max(0, actualNew - expectedNew);
        const start = Math.max(1, stage.runLine + 1);
        const end = Math.max(start, stage.runEndLine + 1);
        const label = start === end ? `Line ${start}` : `Lines ${start}-${end}`;
        const extraLabel = extraCount === 1 ? "variable" : "variables";
        if (expectedNew === 0) {
            return {
                message: `${label} shouldn't add any new variables. Remove the extra ${extraLabel}.`,
                kind: "count",
            };
        }
        const expectedLabel = expectedNew === 1 ? "variable" : "variables";
        return {
            message: `${label} should only add ${expectedNew} new ${expectedLabel}, but you added ${actualNew}. Remove the extra ${extraLabel}.`,
            kind: "count",
        };
    }
    const baselineByName = new Map();
    visibleBaseline.forEach((box) => {
        const name = nameOf(box);
        if (name && !baselineByName.has(name))
            baselineByName.set(name, box);
    });
    const actualByName = new Map();
    visibleActual.forEach((box) => {
        const name = nameOf(box);
        if (name && !actualByName.has(name))
            actualByName.set(name, box);
    });
    let deferredBe = null;
    for (const expectedBox of visibleExpected) {
        const name = nameOf(expectedBox);
        if (!name)
            continue;
        const actualBox = actualByName.get(name);
        if (!actualBox)
            continue;
        const expectedType = typeOf(expectedBox);
        const actualType = typeOf(actualBox);
        if (!sameCType(actualType, expectedType)) {
            return {
                message: `${capitalize(namesPart(name, "type"))} should be $t{${expectedType}}.`,
                kind: "type",
                variable: name,
            };
        }
        const mismatch = !boxValueMatchesSpec(actualBox, expectedBox).ok;
        if (!mismatch)
            continue;
        const expectedValue = (expectedBox.value ?? "").trim();
        const label = expectedValue === "" ? "empty" : `$v{${normalizeZeroDisplay(expectedValue)}}`;
        const baselineBox = baselineByName.get(name);
        const shouldRemain = baselineBox
            ? boxValueMatchesSpec(baselineBox, expectedBox).ok
            : false;
        const message = `${capitalize(namesPart(name, "value"))} should ${shouldRemain ? "remain" : "be"} ${label}.`;
        if (shouldRemain) {
            return {
                message,
                kind: "value",
                variable: name,
            };
        }
        if (!deferredBe) {
            deferredBe = {
                message,
                kind: "value",
                variable: name,
            };
        }
    }
    const actualElementsByName = new Map(comparableStateBoxes(actual)
        .filter(isPartStateBox)
        .map((box) => [box.name, box]));
    const baselineElementsByName = new Map(comparableStateBoxes(baseline)
        .filter(isPartStateBox)
        .map((box) => [box.name, box]));
    for (const expectedBox of comparableStateBoxes(expected).filter(isPartStateBox)) {
        const name = nameOf(expectedBox);
        if (!name)
            continue;
        const actualBox = actualElementsByName.get(name);
        if (!actualBox) {
            return {
                message: `The $n{${name}} ${expectedBox.arrayRoot ? "array element" : "member"} is missing.`,
                kind: "value",
                variable: name,
            };
        }
        const expectedType = typeOf(expectedBox);
        if (!sameCType(typeOf(actualBox), expectedType)) {
            return {
                message: `${capitalize(namesPart(name, "type"))} should be $t{${expectedType}}.`,
                kind: "type",
                variable: name,
            };
        }
        if (boxValueMatchesSpec(actualBox, expectedBox).ok) {
            continue;
        }
        const expectedValue = (expectedBox.value ?? "").trim();
        const label = expectedValue === ""
            ? "empty"
            : `$v{${normalizeZeroDisplay(expectedValue)}}`;
        const baselineBox = baselineElementsByName.get(name);
        const shouldRemain = baselineBox
            ? boxValueMatchesSpec(baselineBox, expectedBox).ok
            : false;
        const message = `${capitalize(namesPart(name, "value"))} should ${shouldRemain ? "remain" : "be"} ${label}.`;
        if (shouldRemain) {
            return { message, kind: "value", variable: name };
        }
        if (!deferredBe) {
            deferredBe = { message, kind: "value", variable: name };
        }
    }
    return deferredBe;
}
