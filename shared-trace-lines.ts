// How far a program's execution has got, line by line: which lines have finished
// running, and which are crossed out because the branches they're in weren't taken.

export type StrikeFragment = { line: number; start: number; end: number };

function ifKeywordColumnsByLine(lines: string[]): Map<number, number[]> {
  const columnsByLine = new Map<number, number[]>();
  let inBlockComment = false;
  for (let lineIndex = 0; lineIndex < lines.length; lineIndex += 1) {
    const line = lines[lineIndex] ?? "";
    const columns: number[] = [];
    let quote: "'" | '"' | null = null;
    let escaped = false;
    for (let column = 0; column < line.length; column += 1) {
      const ch = line[column] ?? "";
      const next = line[column + 1] ?? "";
      if (inBlockComment) {
        if (ch === "*" && next === "/") {
          inBlockComment = false;
          column += 1;
        }
        continue;
      }
      if (quote) {
        if (escaped) escaped = false;
        else if (ch === "\\") escaped = true;
        else if (ch === quote) quote = null;
        continue;
      }
      if (ch === "/" && next === "/") break;
      if (ch === "/" && next === "*") {
        inBlockComment = true;
        column += 1;
        continue;
      }
      if (ch === "'" || ch === '"') {
        quote = ch;
        continue;
      }
      if (ch !== "i" || next !== "f") continue;
      const before = line[column - 1] ?? "";
      const after = line[column + 2] ?? "";
      if (/[A-Za-z0-9_]/.test(before) || /[A-Za-z0-9_]/.test(after)) {
        continue;
      }
      let cursor = column + 2;
      while (/\s/.test(line[cursor] ?? "")) cursor += 1;
      if (line[cursor] === "(") columns.push(column);
    }
    if (columns.length) columnsByLine.set(lineIndex, columns);
  }
  return columnsByLine;
}

// The source with comments and string and character literals blanked out, so braces
// can be matched.
function codeWithoutLiterals(lines: string[]): string[] {
  let inBlockComment = false;
  return lines.map((line) => {
    let out = "";
    let quote: "'" | '"' | null = null;
    for (let column = 0; column < line.length; column += 1) {
      const ch = line[column]!;
      const next = line[column + 1] ?? "";
      if (inBlockComment) {
        if (ch === "*" && next === "/") {
          inBlockComment = false;
          out += "  ";
          column += 1;
        } else out += " ";
      } else if (quote) {
        if (ch === "\\") {
          out += "  ";
          column += 1;
        } else {
          if (ch === quote) quote = null;
          out += " ";
        }
      } else if (ch === "/" && next === "/") {
        break;
      } else if (ch === "/" && next === "*") {
        inBlockComment = true;
        out += "  ";
        column += 1;
      } else if (ch === "'" || ch === '"') {
        quote = ch;
        out += " ";
      } else out += ch;
    }
    return out;
  });
}

// The lines a loop's body spans, given the line the trace reports for the loop: the
// header of a for or while loop, or the closing `} while (...)` of a do-while loop.
function loopBodyLines(code: string[], loopLine: number): { start: number; end: number } {
  const header = code[loopLine] ?? "";
  if (/^\s*}\s*while\b/.test(header)) {
    let depth = 0;
    for (let line = loopLine; line >= 0; line -= 1) {
      const text = code[line]!;
      const from = line === loopLine ? text.indexOf("}") : text.length - 1;
      for (let column = from; column >= 0; column -= 1) {
        if (text[column] === "}") depth += 1;
        else if (text[column] === "{" && --depth === 0) return { start: line, end: loopLine };
      }
    }
    return { start: loopLine, end: loopLine };
  }
  let depth = 0;
  let opened = false;
  for (let line = loopLine; line < code.length; line += 1) {
    for (const ch of code[line]!) {
      if (ch === "{") {
        depth += 1;
        opened = true;
      } else if (ch === "}" && opened && --depth === 0) return { start: loopLine, end: line };
    }
    // A body without braces is the single statement after the header.
    if (!opened && line > loopLine && code[line]!.trim()) return { start: loopLine, end: line };
  }
  return { start: loopLine, end: code.length - 1 };
}

export function strikeFragmentsForTrace(
  lines: string[],
  trace: Array<{
    kind: string;
    file: string;
    startLine: number;
    frameId: number;
    function: string;
    skippedRange: {
      file: string;
      startLine: number;
      startColumn: number;
      endLine: number;
      endColumn: number;
    } | null;
  }>,
  traceEndIndex: number,
): StrikeFragment[] {
  type SkippedRange = NonNullable<(typeof trace)[number]["skippedRange"]>;
  type Loop = { line: number; start: number; end: number };
  type Skip = { range: SkippedRange | null; frameId: number; loop: Loop | null };
  const latestRangeByBranch = new Map<string, Skip>();
  const ifColumns = ifKeywordColumnsByLine(lines);
  const nextBranchSiteByLine = new Map<string, number>();
  const code = codeWithoutLiterals(lines);
  const loops: Loop[] = [...new Set(trace.filter((event) => event.kind === "loop").map((event) => event.startLine))]
    .map((line) => ({ line, ...loopBodyLines(code, line) }));
  // The innermost loop around a line, whose next iteration (or exit) makes a branch
  // inside it irrelevant.
  const innermostLoop = (line: number) =>
    loops
      .filter((loop) => loop.start <= line && line <= loop.end)
      .sort((a, b) => (a.end - a.start) - (b.end - b.start))[0] ?? null;
  for (let index = 0; index <= traceEndIndex && index < trace.length; index += 1) {
    const event = trace[index]!;
    // Leaving the iteration a skipped branch was in, by looping again or leaving
    // the loop, un-skips it.
    latestRangeByBranch.forEach((skip, key) => {
      if (!skip.loop || skip.frameId !== event.frameId) return;
      const line = event.startLine;
      const restarted = event.kind === "loop" && line === skip.loop.line;
      if (restarted || line < skip.loop.start || line > skip.loop.end) latestRangeByBranch.delete(key);
    });
    const lineKey = `${event.file}:${event.startLine}`;
    const loop = innermostLoop(event.startLine);
    // A break out of a switch skips the rest of it.
    if (event.kind !== "branch") {
      if (event.skippedRange) {
        latestRangeByBranch.set(`${event.frameId}:${lineKey}:jump`, { range: event.skippedRange, frameId: event.frameId, loop });
      }
      continue;
    }
    const columns = ifColumns.get(event.startLine) ?? [];
    const range = event.skippedRange;
    let siteIndex = -1;
    if (range?.startLine === event.startLine) {
      siteIndex = columns.indexOf(range.startColumn);
    }
    if (siteIndex < 0 && columns.length === 1) siteIndex = 0;
    if (siteIndex < 0 && columns.length > 1) {
      siteIndex = (nextBranchSiteByLine.get(lineKey) ?? 0) % columns.length;
    }
    if (siteIndex >= 0 && columns.length) {
      nextBranchSiteByLine.set(lineKey, (siteIndex + 1) % columns.length);
    }
    const branchKey = `${event.frameId}:` + (
      siteIndex >= 0
        ? `${lineKey}:${siteIndex}`
        : range
          ? `${lineKey}:${range.startLine}:${range.startColumn}:${range.endLine}:${range.endColumn}`
          : lineKey);
    latestRangeByBranch.set(branchKey, { range, frameId: event.frameId, loop });
  }
  const fragments: StrikeFragment[] = [];
  // Each call skips its own branches.
  const frames = shownFrames(trace, traceEndIndex);
  latestRangeByBranch.forEach(({ range, frameId }) => {
    if (!range || !frames.has(frameId)) return;
    for (let line = range.startLine; line <= range.endLine && line < lines.length; line += 1) {
      const lineText = lines[line] ?? "";
      // Indentation and trailing spaces aren't struck.
      const start = Math.max(line === range.startLine ? range.startColumn : 0, lineText.search(/\S|$/));
      const end = Math.min(line === range.endLine ? range.endColumn : lineText.length, lineText.trimEnd().length);
      if (end > start) fragments.push({ line, start, end });
    }
  });
  return fragments;
}

type TraceEvent = { kind: string; startLine: number; endLine: number; frameId: number; function: string };
type LineRange = { start: number; end: number };

// Each function definition's lines, from its header to its closing brace.
function functionDefinitions(code: string[]): Map<string, LineRange> {
  const definitions = new Map<string, LineRange>();
  let depth = 0;
  let open: { name: string; start: number } | null = null;
  let lastParenLine = -1;
  for (let line = 0; line < code.length; line += 1) {
    const text = code[line]!;
    for (const ch of text) {
      if (depth === 0 && ch === "(") lastParenLine = line;
      if (ch === "{") {
        if (depth === 0 && lastParenLine >= 0) {
          const name = /([A-Za-z_]\w*)\s*\(/.exec(code[lastParenLine]!)?.[1];
          if (name) open = { name, start: lastParenLine };
        }
        depth += 1;
      } else if (ch === "}") {
        depth -= 1;
        if (depth === 0) {
          if (open) definitions.set(open.name, { start: open.start, end: line });
          open = null;
          lastParenLine = -1;
        }
      } else if (ch === ";" && depth === 0) {
        lastParenLine = -1;
      }
    }
  }
  return definitions;
}

// main's frame. A call event belongs to the function being called, so it's the
// first other event in main.
function mainFrame(trace: Array<{ kind: string; frameId: number; function: string }>): number | null {
  return (trace.find((event) => event.function === "main" && event.kind !== "call") ?? trace[0])?.frameId ?? null;
}

/**
 * The calls whose lines are shown: for each function, its innermost call that's
 * still running, or else the call that finished most recently. A program without
 * function calls only has main's.
 */
function shownFrames(
  trace: Array<{ kind: string; frameId: number; function: string }>,
  traceEndIndex: number,
): Set<number> {
  const functionOf = new Map<number, string>();
  const active: number[] = [];
  const lastFinished = new Map<string, number>();
  const main = mainFrame(trace);
  if (main !== null) {
    active.push(main);
    functionOf.set(main, "main");
  }
  for (let index = 0; index <= traceEndIndex && index < trace.length; index += 1) {
    const event = trace[index]!;
    if (event.kind === "call") {
      active.push(event.frameId);
      functionOf.set(event.frameId, event.function);
    } else if (event.kind === "return" && active.length > 1) {
      const callee = active.pop()!;
      lastFinished.set(functionOf.get(callee) ?? "", callee);
    }
  }
  const shown = new Map<string, number>(lastFinished);
  for (const frame of active) shown.set(functionOf.get(frame) ?? "", frame);
  return new Set(shown.values());
}

/**
 * The lines that have finished running by the end of `trace[traceEndIndex]`: in the
 * current iteration of any loop they're in, and the current call of any function
 * they're in. A loop or a function call that has finished counts as finished as a
 * whole, apart from lines it never ran. Lines that never run, like declarations of
 * functions, never count.
 */
export function doneLinesForTrace(
  lines: string[],
  trace: TraceEvent[],
  traceEndIndex: number,
): Set<number> {
  const code = codeWithoutLiterals(lines);
  const functions = functionDefinitions(code);
  const loopBodies = new Map<number, LineRange>();
  const loopBody = (line: number) => {
    let body = loopBodies.get(line);
    if (!body) loopBodies.set(line, (body = loopBodyLines(code, line)));
    return body;
  };
  // Lines that just open a block, with the block's last line. A block's opening
  // brace counts as run once execution is inside the block.
  const openers: Array<{ line: number; end: number }> = [];
  code.forEach((text, line) => {
    if (text.trim() !== "{") return;
    let depth = 0;
    for (let end = line; end < code.length; end += 1) {
      for (const ch of code[end]!) {
        if (ch === "{") depth += 1;
        else if (ch === "}") depth -= 1;
      }
      if (depth <= 0) {
        openers.push({ line, end });
        return;
      }
    }
  });
  // Each call has its own lines, so a recursive call doesn't affect its caller's.
  const doneByFrame = new Map<number, Set<number>>();
  const ranByFrame = new Map<number, Set<number>>();
  const setOf = (sets: Map<number, Set<number>>, frame: number) => {
    let set = sets.get(frame);
    if (!set) sets.set(frame, (set = new Set()));
    return set;
  };
  const finish = (frame: number, start: number, end: number) => {
    const done = setOf(doneByFrame, frame);
    const ran = setOf(ranByFrame, frame);
    for (const opener of openers) {
      if (start > opener.line && start <= opener.end) {
        done.add(opener.line);
        ran.add(opener.line);
      }
    }
    for (let line = start; line <= end; line += 1) {
      done.add(line);
      ran.add(line);
    }
  };
  const forget = (frame: number, range: LineRange, except: number) => {
    const done = setOf(doneByFrame, frame);
    for (let line = range.start; line <= range.end; line += 1) if (line !== except) done.delete(line);
  };
  // A loop that has finished has finished as a whole.
  const finishLoop = (frame: number, body: LineRange) => {
    const done = setOf(doneByFrame, frame);
    const ran = setOf(ranByFrame, frame);
    for (let line = body.start; line <= body.end; line += 1) if (ran.has(line)) done.add(line);
  };
  // The loops each frame is currently inside, innermost last.
  const activeLoops = new Map<number, Array<{ line: number; body: LineRange }>>();
  const leaveLoops = (frame: number, line: number) => {
    const loops = activeLoops.get(frame) ?? [];
    while (loops.length) {
      const loop = loops[loops.length - 1]!;
      if (line >= loop.body.start && line <= loop.body.end) break;
      loops.pop();
      finishLoop(frame, loop.body);
    }
    activeLoops.set(frame, loops);
  };
  // Entering a function: its header has run, and nothing else in this call has yet.
  const enter = (name: string, frame: number) => {
    const definition = functions.get(name);
    if (!definition) return;
    let brace = definition.start;
    while (brace < definition.end && !code[brace]!.includes("{")) brace += 1;
    finish(frame, definition.start, brace);
  };
  // main is entered when the program starts.
  const main = mainFrame(trace);
  if (traceEndIndex >= 0 && main !== null) enter("main", main);
  for (let index = 0; index <= traceEndIndex && index < trace.length; index += 1) {
    const event = trace[index]!;
    const frame = event.frameId;
    if (event.kind === "call") {
      enter(event.function, frame);
      continue;
    }
    // Back in the caller, the line with the call still has to finish.
    if (event.kind === "return") continue;
    leaveLoops(frame, event.startLine);
    if (event.kind === "loop") {
      const body = loopBody(event.startLine);
      const loops = activeLoops.get(frame) ?? [];
      if (loops[loops.length - 1]?.line !== event.startLine) loops.push({ line: event.startLine, body });
      activeLoops.set(frame, loops);
      // Checking the condition starts a new iteration.
      forget(frame, body, event.startLine);
      finish(frame, event.startLine, event.endLine);
      const next = trace.slice(index + 1).find((later) => later.frameId === frame);
      if (!next || next.startLine < body.start || next.startLine > body.end) {
        loops.pop();
        finishLoop(frame, body);
      }
      continue;
    }
    finish(frame, event.startLine, event.endLine);
  }
  const done = new Set<number>();
  for (const frame of shownFrames(trace, traceEndIndex)) {
    for (const line of doneByFrame.get(frame) ?? []) done.add(line);
  }
  return done;
}
