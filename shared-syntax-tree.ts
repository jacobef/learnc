// Hand-built diagrams of how code is parsed, drawn as top-down trees.

export interface SyntaxTreeNode {
  // What the node is, like "*", "[3]", or "x". Styled tokens like $c{...} aren't used here.
  text: string;
  kind?: "operator" | "name" | "type" | "plain";
  // A short description shown under the node, like its type.
  note?: string;
  // Nodes with the same pair number, in different diagrams, are drawn in the same
  // color, to show that they correspond.
  pair?: number;
  children?: SyntaxTreeNode[];
}

export interface SyntaxDiagram {
  title: string;
  tree: SyntaxTreeNode;
}

function renderNode(node: SyntaxTreeNode): HTMLLIElement {
  const item = document.createElement("li");
  const box = document.createElement("div");
  box.className = `syntax-tree-node syntax-tree-${node.kind ?? "plain"}`;
  if (node.pair) box.classList.add(`syntax-tree-pair-${node.pair}`);
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
    for (const child of node.children) list.appendChild(renderNode(child));
    item.appendChild(list);
  }
  return item;
}

/** Draws diagrams side by side, each with its title above it. */
export function renderSyntaxDiagrams(diagrams: SyntaxDiagram[]): HTMLElement {
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
