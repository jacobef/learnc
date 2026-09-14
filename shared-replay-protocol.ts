// Deliberately no account, session, device, URL, or wall-clock fields.
export interface ReplayNode {
  id: number;
  tag?: string;
  text?: string;
  attrs?: Record<string, string>;
  value?: string;
  children?: ReplayNode[];
}

export interface ReplayEvent {
  t: number;
  type: "start" | "patch" | "input" | "click" | "focus" | "scroll" | "layout" | "check";
  data: Record<string, unknown>;
}

export interface LevelReplay {
  version: 1;
  level: string;
  events: ReplayEvent[];
}

export const REPLAY_ATTRIBUTES = [
  "class", "hidden", "disabled", "readonly", "data-role", "colspan", "rowspan",
  "aria-pressed", "aria-selected", "aria-expanded", "title", "placeholder",
];
export const REPLAY_TAGS = new Set([
  "div", "span", "section", "p", "h1", "h2", "h3", "button", "textarea", "input",
  "label", "select", "option", "pre", "code", "br", "strong", "b", "em", "i",
  "small", "sub", "sup", "ul", "ol", "li", "table", "thead", "tbody", "tr",
  "td", "th", "details", "summary", "a", "hr",
]);
export const REPLAY_LIMIT_BYTES = 8 * 1024 * 1024;
export const REPLAY_LIMIT_EVENTS = 50_000;
