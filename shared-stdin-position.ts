/** Map the interpreter's byte position to a caret in the editable UTF-16 text. */
export function stdinReadPosition(input: string, position: number) {
  const eof = input.search(/[\u0004\u2404]/u);
  const available = eof < 0 ? input : input.slice(0, eof);
  const target = Math.max(0, Math.floor(position));
  let bytes = 0;
  let offset = 0;
  let line = 1;
  let column = 1;
  let partialByte = 0;
  for (const character of available) {
    const codePoint = character.codePointAt(0)!;
    const width = codePoint <= 0x7f ? 1 : codePoint <= 0x7ff ? 2 : codePoint <= 0xffff ? 3 : 4;
    if (bytes + width > target) {
      partialByte = target - bytes;
      break;
    }
    bytes += width;
    offset += character.length;
    if (character === "\n") {
      line += 1;
      column = 1;
    } else {
      column += 1;
    }
  }
  const atEnd = offset === available.length;
  const location = `line ${line}, column ${column}`;
  const detail = partialByte > 0 ? ` (byte ${partialByte + 1} of this character)` : "";
  const label = atEnd
    ? eof < 0 ? `End of available input · ${location}` : `End of input (EOF) · ${location}`
    : `Next read: ${location}${detail}`;
  return { offset, label };
}
