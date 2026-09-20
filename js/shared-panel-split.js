/** Drag to allocate horizontal space between two grid panels. */
export function bindPanelSplit(container, separator, initialShare, onChange) {
    const defaultShare = 1 / 2.3;
    let share = defaultShare;
    let requestedShare = initialShare;
    let drag = null;
    function availableWidth() {
        const gap = parseFloat(getComputedStyle(container).columnGap) || 0;
        return container.clientWidth - separator.offsetWidth - gap * 2;
    }
    function apply(value) {
        requestedShare = Number.isFinite(value) ? value : defaultShare;
        // Keep a usable editor and expression input even at either end of the drag.
        // Recalculate after a window/sidebar resize as well as pointer movement.
        const width = availableWidth();
        const minimumShare = width > 0 ? Math.min(0.5, Math.max(0.2, 280 / width)) : 0.2;
        share = Math.max(minimumShare, Math.min(1 - minimumShare, requestedShare));
        container.style.setProperty("--code-column", `${share}fr`);
        container.style.setProperty("--state-column", `${1 - share}fr`);
    }
    separator.addEventListener("pointerdown", event => {
        if (event.button !== 0 || !event.isPrimary || drag)
            return;
        event.preventDefault();
        separator.setPointerCapture(event.pointerId);
        drag = { pointer: event.pointerId, x: event.clientX, share };
        container.classList.add("is-resizing-panels");
    });
    separator.addEventListener("pointermove", event => {
        if (!drag || event.pointerId !== drag.pointer)
            return;
        const available = availableWidth();
        if (available > 0)
            apply(drag.share + (event.clientX - drag.x) / available);
    });
    function finish(event) {
        if (!drag || event.pointerId !== drag.pointer)
            return;
        drag = null;
        container.classList.remove("is-resizing-panels");
        if (separator.hasPointerCapture(event.pointerId))
            separator.releasePointerCapture(event.pointerId);
        onChange(share);
    }
    separator.addEventListener("pointerup", finish);
    separator.addEventListener("pointercancel", finish);
    separator.addEventListener("lostpointercapture", finish);
    separator.addEventListener("dblclick", () => {
        apply(defaultShare);
        onChange(share);
    });
    new ResizeObserver(() => apply(requestedShare)).observe(container);
    apply(initialShare);
}
