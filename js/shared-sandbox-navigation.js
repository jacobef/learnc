/** Position p is the boundary before trace[p], or the terminal boundary. */
export function executionBoundaries(trace, mainClose, blocked, end, paused = false) {
    const last = trace[trace.length - 1];
    const terminal = blocked
        ? { file: blocked.file, line: blocked.startLine }
        : paused ? null
            : mainClose && (!last || last.file !== mainClose.file || mainClose.line > last.endLine)
                ? mainClose
                : end;
    return [...trace.map(event => ({ file: event.file, line: event.startLine, kind: event.kind })), terminal];
}
export function boundaryOccurrences(boundaries, file) {
    const result = new Map();
    boundaries.forEach((boundary, position) => {
        if (!boundary || boundary.file !== file)
            return;
        const positions = result.get(boundary.line) ?? [];
        positions.push(position);
        result.set(boundary.line, positions);
    });
    return result;
}
export function nearestOccurrence(positions, current, direction) {
    if (direction === "forward")
        return positions.find(position => position > current);
    for (let i = positions.length - 1; i >= 0; i--) {
        if (positions[i] < current)
            return positions[i];
    }
    return undefined;
}
export function defaultOccurrence(positions, current) {
    return nearestOccurrence(positions, current, "forward") ?? nearestOccurrence(positions, current, "backward");
}
export function canJumpToBoundary(positions, current, ask) {
    if (positions.includes(current))
        return ask && positions.length > 1;
    return positions.length > 0;
}
/** Both loop controls navigate the same ordinary line occurrences. */
export function boundaryTargets(boundaries, file) {
    const loopLines = new Set(boundaries.filter(boundary => boundary?.file === file && boundary.kind === "loop").map(boundary => boundary.line));
    return [...boundaryOccurrences(boundaries, file)].flatMap(([line, positions]) => {
        const paired = loopLines.has(line);
        const nearest = { line, kind: "nearest", positions, paired };
        return paired ? [{ line, kind: "first", positions, paired }, nearest] : [nearest];
    });
}
