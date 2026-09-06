export function queryElement(selector, root = document) {
    return (root?.querySelector?.(selector) ?? null);
}
export function queryRole(role, root = document) {
    return queryElement(`[data-role="${role}"]`, root);
}
export function clearNode(node) {
    node?.replaceChildren();
}
export function el(html) {
    const t = document.createElement("template");
    t.innerHTML = html.trim();
    return t.content.firstElementChild;
}
export function txt(n) {
    return (n?.textContent || "").trim();
}
export function disableAutoText(el) {
    if (!el || el.nodeType !== 1)
        return;
    el.setAttribute("autocapitalize", "off");
    el.setAttribute("autocorrect", "off");
    el.setAttribute("autocomplete", "off");
    el.setAttribute("spellcheck", "false");
}
