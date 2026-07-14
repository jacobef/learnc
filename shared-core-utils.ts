export type CTypeKind =
  | "void"
  | "integer"
  | "floating"
  | "complex"
  | "pointer"
  | "array"
  | "aggregate"
  | "function"
  | "va-list"
  | "unknown";

export interface CTypeInfo {
  kind: CTypeKind;
  pointerDepth: number;
  arrayShape: number[];
  pointeeArrayShape: number[];
  size: number | null;
  align: number | null;
}
export type BoxValue = string;
export interface BoxState {
  name: string;
  type: string;
  value: BoxValue;
  displayValue?: string | null;
  exactValue?: string | null;
  rawValue?: string | null;
  address?: string | null;
  arrayRoot?: string | null;
  arrayShape?: number[] | null;
  arrayIndices?: number[] | null;
  aliases?: string[] | null;
  typeInfo?: CTypeInfo | null;
  names?: string[] | string | null;
  allowNameEdit?: boolean | null;
  allowTypeEdit?: boolean | null;
  allowDelete?: boolean | null;
  showDoubleExact?: boolean | null;
  dynamicAddress?: boolean | null;
  defaultAddressType?: string | null;
  expectedAddress?: string | null;
  expectedAddressType?: string | null;
  node?: HTMLElement;
}

export type ProgramDiagnosticRange = {
  startLine: number;
  startCol: number;
  endLine: number;
  endCol: number;
};

export type ProgramDiagnostic = {
  kind: "compile" | "ub";
  message: string;
  file?: string;
  tip?: string;
  range: ProgramDiagnosticRange;
};

export function normalizeZeroDisplay(value: BoxValue): string {
  const trimmed = value.trim();
  if (trimmed === "-0") return "0";
  return trimmed;
}

export type TextTokenReplacement = readonly [string, string];

export function replaceTextTokens(
  text: string,
  replacements: ReadonlyArray<TextTokenReplacement>,
): string {
  let out = String(text);
  for (const [needle, replacement] of replacements) {
    if (!needle) continue;
    out = out.split(needle).join(replacement);
  }
  return out;
}

export function applyTextTokenReplacements(
  value: string | string[] | null | undefined,
  replacements: ReadonlyArray<TextTokenReplacement>,
): string | string[] | null | undefined {
  if (value == null) return value;
  if (typeof value === "string") {
    return replaceTextTokens(value, replacements);
  }
  return value.map((part) => replaceTextTokens(part, replacements));
}

export function cloneBoxes(list: BoxState[] | null | undefined): BoxState[] {
  if (!Array.isArray(list)) return [];
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
