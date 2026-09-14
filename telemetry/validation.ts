// Reject metadata we do not collect, even if a modified client sends it.
const tags = new Set(["div", "span", "section", "p", "h1", "h2", "h3", "button", "textarea", "input", "label", "select", "option", "pre", "code", "br", "strong", "b", "em", "i", "small", "sub", "sup", "ul", "ol", "li", "table", "thead", "tbody", "tr", "td", "th", "details", "summary", "a", "hr"]);
const attributes = new Set(["class", "hidden", "disabled", "readonly", "data-role", "colspan", "rowspan", "aria-pressed", "aria-selected", "aria-expanded", "title", "placeholder"]);
export const MAX_BYTES = 8 * 1024 * 1024;

function object(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
function keys(value: Record<string, unknown>, allowed: string[]) {
  return Object.keys(value).every(key => allowed.includes(key));
}
function integer(value: unknown, max = 1_000_000) {
  return typeof value === "number" && Number.isInteger(value) && value >= 0 && value <= max;
}
function text(value: unknown, max = 100_000) {
  return typeof value === "string" && value.length <= max;
}
function node(value: unknown, depth = 0): boolean {
  if (depth > 80 || !object(value) || !integer(value.id) || value.id === 0) return false;
  if ("text" in value) return keys(value, ["id", "text"]) && text(value.text);
  if (!keys(value, ["id", "tag", "attrs", "value", "children"]) || !tags.has(value.tag as string)) return false;
  if (!object(value.attrs) || !Object.entries(value.attrs).every(([key, val]) => attributes.has(key) && text(val, 1000))) return false;
  if (value.value !== undefined && !text(value.value)) return false;
  return Array.isArray(value.children) && value.children.length <= 20_000 && value.children.every(child => node(child, depth + 1));
}

export function validReplay(value: unknown): boolean {
  if (!object(value) || !keys(value, ["version", "level", "events"]) || value.version !== 1) return false;
  if (typeof value.level !== "string" || !/^\d{1,3}-[a-z0-9-]{1,80}\.html$/.test(value.level)) return false;
  if (!Array.isArray(value.events) || value.events.length < 2 || value.events.length > 50_000) return false;
  let previous = -1;
  for (const [index, event] of value.events.entries()) {
    if (!object(event) || !keys(event, ["t", "type", "data"]) || !integer(event.t, 7 * 24 * 60 * 60 * 1000) || !object(event.data)) return false;
    if ((event.t as number) < previous) return false;
    previous = event.t as number;
    const d = event.data;
    switch (event.type) {
      case "start":
        if (index !== 0 || !keys(d, ["root", "mobile"]) || !node(d.root) || typeof d.mobile !== "boolean") return false;
        break;
      case "patch":
        if (!keys(d, ["nodes"]) || !Array.isArray(d.nodes) || d.nodes.length > 20_000 || !d.nodes.every(n => node(n))) return false;
        break;
      case "input":
        if (!keys(d, ["target", "value"]) || !integer(d.target) || !text(d.value)) return false;
        break;
      case "click": case "focus": case "check":
        if (!keys(d, ["target"]) || !integer(d.target)) return false;
        break;
      case "scroll":
        if (!keys(d, ["target", "x", "y"]) || !integer(d.target) || !integer(d.x) || !integer(d.y)) return false;
        break;
      case "layout":
        if (!keys(d, ["mobile"]) || typeof d.mobile !== "boolean") return false;
        break;
      default: return false;
    }
  }
  return value.events[0].type === "start" && value.events.at(-1).type === "check";
}
