type QueryRoot = ParentNode | null | undefined;

export function queryElement<T extends Element>(
  selector: string,
  root: QueryRoot = document,
): T | null {
  return (root?.querySelector?.(selector) ?? null) as T | null;
}

export function queryRole<T extends Element>(
  role: string,
  root: QueryRoot = document,
): T | null {
  return queryElement<T>(`[data-role="${role}"]`, root);
}

export function clearNode(node: Element | null | undefined): void {
  node?.replaceChildren();
}

export function el(html: string): HTMLElement {
  const t = document.createElement("template");
  t.innerHTML = html.trim();
  return t.content.firstElementChild as HTMLElement;
}

export function txt(n?: Node | null): string {
  return (n?.textContent || "").trim();
}

export function disableAutoText(el?: Element | null) {
  if (!el || el.nodeType !== 1) return;
  el.setAttribute("autocapitalize", "off");
  el.setAttribute("autocorrect", "off");
  el.setAttribute("autocomplete", "off");
  el.setAttribute("spellcheck", "false");
}
