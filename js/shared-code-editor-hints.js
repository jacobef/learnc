function joinEnglish(items) {
    if (items.length <= 1)
        return items[0] || "";
    if (items.length === 2)
        return `${items[0]} and ${items[1]}`;
    return `${items.slice(0, -1).join(", ")}, and ${items[items.length - 1]}`;
}
export function formatNames(names) {
    return joinEnglish(names.map((name) => `$n{${name}}`));
}
export function boxesByName(boxes) {
    return new Map(boxes.map((box) => [box.name, box]));
}
