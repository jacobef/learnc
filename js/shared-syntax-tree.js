// Hand-built diagrams of how code is parsed, drawn as top-down trees.
function renderNode(node) {
    const item = document.createElement("li");
    const box = document.createElement("div");
    box.className = `syntax-tree-node syntax-tree-${node.kind ?? "plain"}`;
    if (node.pair)
        box.classList.add(`syntax-tree-pair-${node.pair}`);
    const text = document.createElement("code");
    text.textContent = node.text;
    box.appendChild(text);
    if (node.note) {
        const note = document.createElement("div");
        note.className = "syntax-tree-note";
        note.textContent = node.note;
        box.appendChild(note);
    }
    item.appendChild(box);
    if (node.children?.length) {
        const list = document.createElement("ul");
        for (const child of node.children)
            list.appendChild(renderNode(child));
        item.appendChild(list);
    }
    return item;
}
/** Draws diagrams side by side, each with its title above it. */
export function renderSyntaxDiagrams(diagrams) {
    const wrap = document.createElement("div");
    wrap.className = "syntax-diagrams";
    for (const diagram of diagrams) {
        const figure = document.createElement("figure");
        figure.className = "syntax-diagram";
        const caption = document.createElement("figcaption");
        caption.textContent = diagram.title;
        const tree = document.createElement("ul");
        tree.className = "syntax-tree";
        tree.appendChild(renderNode(diagram.tree));
        figure.append(caption, tree);
        wrap.appendChild(figure);
    }
    return wrap;
}
