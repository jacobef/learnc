type TraceLocation = { file: string; startLine: number; endLine: number; kind?: string };
export type LineBoundary = { file: string; line: number; kind?: string };

/** Position p is the boundary before trace[p], or the terminal boundary. */
export function executionBoundaries(
  trace: readonly TraceLocation[],
  mainClose: LineBoundary | null,
  blocked: { file: string; startLine: number } | null,
  end: LineBoundary,
  paused = false,
): (LineBoundary | null)[] {
  const last = trace[trace.length - 1];
  const terminal = blocked
    ? { file: blocked.file, line: blocked.startLine }
    : paused ? null
    : mainClose && (!last || last.file !== mainClose.file || mainClose.line > last.endLine)
      ? mainClose
      : end;
  return [...trace.map(event => ({ file: event.file, line: event.startLine, kind: event.kind })), terminal];
}

export function boundaryOccurrences(boundaries: readonly (LineBoundary | null)[], file: string): Map<number, number[]> {
  const result = new Map<number, number[]>();
  boundaries.forEach((boundary, position) => {
    if (!boundary || boundary.file !== file) return;
    const positions = result.get(boundary.line) ?? [];
    positions.push(position);
    result.set(boundary.line, positions);
  });
  return result;
}

export function nearestOccurrence(positions: readonly number[], current: number, direction: "forward" | "backward"): number | undefined {
  if (direction === "forward") return positions.find(position => position > current);
  for (let i = positions.length - 1; i >= 0; i--) {
    if (positions[i]! < current) return positions[i];
  }
  return undefined;
}

export function defaultOccurrence(positions: readonly number[], current: number): number | undefined {
  return nearestOccurrence(positions, current, "forward") ?? nearestOccurrence(positions, current, "backward");
}

export function canJumpToBoundary(positions: readonly number[], current: number, ask: boolean): boolean {
  if (positions.includes(current)) return ask && positions.length > 1;
  return positions.length > 0;
}

export type BoundaryTarget = {
  line: number;
  kind: "first" | "nearest";
  positions: number[];
  paired: boolean;
};

/** Both loop controls navigate the same ordinary line occurrences. */
export function boundaryTargets(boundaries: readonly (LineBoundary | null)[], file: string): BoundaryTarget[] {
  const loopLines = new Set(boundaries.filter(boundary => boundary?.file === file && boundary.kind === "loop").map(boundary => boundary!.line));
  return [...boundaryOccurrences(boundaries, file)].flatMap(([line, positions]) => {
    const paired = loopLines.has(line);
    const nearest: BoundaryTarget = {line, kind: "nearest", positions, paired};
    return paired ? [{line, kind: "first" as const, positions, paired}, nearest] : [nearest];
  });
}
