function joinEnglish(items) {
    if (items.length <= 1)
        return items[0] || "";
    if (items.length === 2)
        return `${items[0]} and ${items[1]}`;
    return `${items.slice(0, -1).join(", ")}, and ${items[items.length - 1]}`;
}
export function missingSemicolonHint(lines) {
    if (!lines.length)
        return null;
    const lineText = joinEnglish(lines.map(String));
    const semicolonText = lines.length === 1 ? "a semicolon" : "semicolons";
    const lineLabel = lines.length === 1 ? "line" : "lines";
    return `Add ${semicolonText} at the end of ${lineLabel} ${lineText}.`;
}
export function formatNames(names) {
    return joinEnglish(names.map((name) => `$n{${name}}`));
}
export function boxesByName(boxes) {
    return new Map(boxes.map((box) => [box.name, box]));
}
export function firstRedeclaredName(statements) {
    const declared = new Set();
    for (const stmt of statements) {
        if (stmt.kind !== "decl" && stmt.kind !== "declAssign")
            continue;
        if (declared.has(stmt.name))
            return stmt.name;
        declared.add(stmt.name);
    }
    return null;
}
