/** Unwinding a frame and completing its caller's expression are one visible step. */
function visibleStepBoundaries(trace) {
    let phase = "body";
    const visible = [];
    for (const event of trace) {
        visible.push(event.kind === "ub" || (event.kind !== "return" &&
            (phase === "body" || (phase === "resume" && event.kind === "call"))));
        if (event.kind === "function-exit")
            phase = "unwind";
        else if (event.kind === "return")
            phase = "resume";
        else if (phase === "resume" || event.kind === "ub")
            phase = "body";
    }
    visible.push(phase === "body");
    return visible;
}
export function previousVisibleStepPosition(trace, position) {
    const visible = visibleStepBoundaries(trace);
    for (let i = Math.min(position - 1, trace.length); i > 0; i--) {
        if (visible[i])
            return i;
    }
    return 0;
}
export function stepDepth(trace, position) {
    return position > 0 ? trace[position - 1]?.callDepth ?? 0 : 0;
}
/** Return null when the requested boundary has not yet been captured. */
export function stepDestination(trace, position, mode, depth = stepDepth(trace, position)) {
    const visible = visibleStepBoundaries(trace);
    let reached = false;
    for (let i = position; i < trace.length; i++) {
        const event = trace[i];
        if (event.kind === "ub")
            return i + 1;
        reached || (reached = mode === "into" || event.callDepth < depth ||
            (mode === "over" && event.callDepth === depth && event.kind !== "call" && event.kind !== "return"));
        if (reached && visible[i + 1])
            return i + 1;
    }
    return null;
}
/** Position p is the boundary before trace[p], or the terminal boundary. */
export function executionBoundaries(trace, mainClose, blocked, end, paused = false) {
    const last = trace[trace.length - 1];
    const terminal = blocked
        ? { file: blocked.file, line: blocked.startLine }
        : paused ? null
            : mainClose && (!last || last.file !== mainClose.file || mainClose.line > last.endLine)
                ? mainClose
                : end;
    const visible = visibleStepBoundaries(trace);
    return [...trace.map((event, i) => visible[i] ? { file: event.file, line: event.startLine, kind: event.kind } : null), terminal];
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
