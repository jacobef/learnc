import { REPLAY_ATTRIBUTES, REPLAY_TAGS, REPLAY_LIMIT_BYTES, REPLAY_LIMIT_EVENTS } from "./shared-replay-protocol.js";
const fileInput = document.querySelector("#replay-file");
const play = document.querySelector("#replay-play");
const speed = document.querySelector("#replay-speed");
const position = document.querySelector("#replay-position");
const status = document.querySelector("#replay-status");
const time = document.querySelector("#replay-time");
const stage = document.querySelector("#replay-stage");
const nodes = new Map();
let replay = null;
let cursor = 0;
let elapsed = 0;
let playing = false;
let lastFrame = 0;
function makeNode(record, depth = 0) {
    if (depth > 80 || !record || !Number.isInteger(record.id))
        throw new Error("Invalid replay node");
    let node;
    if (typeof record.text === "string")
        node = document.createTextNode(record.text);
    else {
        if (!record.tag || !REPLAY_TAGS.has(record.tag))
            throw new Error("Unsupported replay element");
        const element = document.createElement(record.tag);
        for (const [name, value] of Object.entries(record.attrs || {})) {
            if (REPLAY_ATTRIBUTES.includes(name) && typeof value === "string")
                element.setAttribute(name, value);
        }
        for (const child of record.children || [])
            element.appendChild(makeNode(child, depth + 1));
        if (element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement) {
            element.value = record.value || "";
            element.readOnly = true;
        }
        else if (element instanceof HTMLSelectElement)
            element.value = record.value || "";
        node = element;
    }
    nodes.set(record.id, node);
    return node;
}
function apply(event) {
    const data = event.data;
    if (event.type === "start") {
        nodes.clear();
        stage.replaceChildren(makeNode(data.root));
        stage.classList.toggle("is-mobile", data.mobile === true);
    }
    else if (event.type === "patch") {
        for (const record of data.nodes) {
            const old = nodes.get(record.id);
            if (old?.parentNode)
                old.parentNode.replaceChild(makeNode(record), old);
        }
    }
    else if (event.type === "input") {
        const target = nodes.get(data.target);
        if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement)
            target.value = String(data.value);
        else if (target)
            target.textContent = String(data.value);
    }
    else if (event.type === "click" || event.type === "focus" || event.type === "check") {
        stage.querySelectorAll(".replay-focus").forEach(node => node.classList.remove("replay-focus"));
        const target = nodes.get(data.target);
        if (target instanceof HTMLElement)
            target.classList.add("replay-focus");
    }
    else if (event.type === "scroll") {
        const target = data.target === 0 ? stage : nodes.get(data.target);
        if (target instanceof HTMLElement)
            target.scrollTo(Number(data.x), Number(data.y));
    }
    else if (event.type === "layout")
        stage.classList.toggle("is-mobile", data.mobile === true);
}
function seek(ms) {
    if (!replay)
        return;
    if (ms < elapsed) {
        cursor = 0;
        nodes.clear();
        stage.replaceChildren();
    }
    elapsed = ms;
    while (cursor < replay.events.length && replay.events[cursor].t <= ms)
        apply(replay.events[cursor++]);
    position.value = String(ms);
    time.value = `${Math.floor(ms / 60000)}:${(ms / 1000 % 60).toFixed(1).padStart(4, "0")}`;
}
function pause() { playing = false; play.textContent = "Play"; }
function frame(now) {
    if (!playing || !replay)
        return;
    const end = replay.events[replay.events.length - 1].t;
    seek(Math.min(end, elapsed + (now - lastFrame) * Number(speed.value)));
    lastFrame = now;
    if (elapsed >= end)
        pause();
    else
        requestAnimationFrame(frame);
}
play.addEventListener("click", () => {
    if (playing) {
        pause();
        return;
    }
    if (elapsed >= Number(position.max))
        seek(0);
    playing = true;
    play.textContent = "Pause";
    lastFrame = performance.now();
    requestAnimationFrame(frame);
});
position.addEventListener("input", () => { pause(); seek(Number(position.value)); });
fileInput.addEventListener("change", async () => {
    pause();
    const file = fileInput.files?.[0];
    if (!file)
        return;
    try {
        if (file.size > REPLAY_LIMIT_BYTES)
            throw new Error("Replay file is too large");
        const stream = file.name.endsWith(".gz") ? file.stream().pipeThrough(new DecompressionStream("gzip")) : file.stream();
        const reader = stream.getReader();
        const chunks = [];
        let size = 0;
        for (;;) {
            const { value, done } = await reader.read();
            if (done)
                break;
            size += value.length;
            if (size > REPLAY_LIMIT_BYTES) {
                await reader.cancel();
                throw new Error("Replay file is too large");
            }
            chunks.push(value);
        }
        const content = new Uint8Array(size);
        let offset = 0;
        for (const chunk of chunks) {
            content.set(chunk, offset);
            offset += chunk.length;
        }
        const parsed = JSON.parse(new TextDecoder().decode(content));
        if (parsed.version !== 1 || typeof parsed.level !== "string" || !Array.isArray(parsed.events) || parsed.events.length < 2 || parsed.events.length > REPLAY_LIMIT_EVENTS || parsed.events[0].type !== "start" || parsed.events[parsed.events.length - 1].type !== "check")
            throw new Error("Not a complete level replay");
        let previous = -1;
        for (const event of parsed.events) {
            if (!Number.isInteger(event.t) || event.t < previous)
                throw new Error("Invalid replay timing");
            previous = event.t;
        }
        replay = parsed;
        cursor = 0;
        elapsed = 0;
        position.max = String(previous);
        play.disabled = position.disabled = false;
        status.textContent = `${replay.level} · ${replay.events.filter(event => event.type === "check").length} Check clicks · elapsed time only`;
        seek(0);
    }
    catch (error) {
        replay = null;
        stage.replaceChildren();
        nodes.clear();
        play.disabled = position.disabled = true;
        status.textContent = error instanceof Error ? error.message : "Could not open replay";
    }
});
