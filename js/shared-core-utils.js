export function normalizeZeroDisplay(value) {
    const trimmed = value.trim();
    if (trimmed === "-0")
        return "0";
    return trimmed;
}
export function replaceTextTokens(text, replacements) {
    let out = String(text);
    for (const [needle, replacement] of replacements) {
        if (!needle)
            continue;
        out = out.split(needle).join(replacement);
    }
    return out;
}
export function applyTextTokenReplacements(value, replacements) {
    if (value == null)
        return value;
    if (typeof value === "string") {
        return replaceTextTokens(value, replacements);
    }
    return value.map((part) => replaceTextTokens(part, replacements));
}
export function cloneBoxes(list) {
    if (!Array.isArray(list))
        return [];
    return list.map((box) => ({
        ...box,
        names: Array.isArray(box.names)
            ? [...box.names]
            : [box.names || box.name].filter(Boolean),
        arrayShape: box.arrayShape ? [...box.arrayShape] : box.arrayShape,
        arrayIndices: box.arrayIndices ? [...box.arrayIndices] : box.arrayIndices,
        aliases: box.aliases ? [...box.aliases] : box.aliases,
        typeInfo: box.typeInfo
            ? {
                ...box.typeInfo,
                arrayShape: [...box.typeInfo.arrayShape],
                pointeeArrayShape: [...box.typeInfo.pointeeArrayShape],
            }
            : box.typeInfo,
    }));
}
