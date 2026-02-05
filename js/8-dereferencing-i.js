import { $, ensureBaseLayout, flashStatus, getNavLabelForHref, randAddr, resolveActiveNavItem, setPartsContent, vbox, } from "./shared-core.js";
const { main } = ensureBaseLayout();
main.classList.add("main-panelized");
const activeItem = resolveActiveNavItem();
const resolvedTitle = activeItem?.label || "";
if (resolvedTitle) {
    document.title = `C Boxes - ${resolvedTitle}`;
    const heading = document.querySelector(".page-title");
    if (heading)
        heading.textContent = resolvedTitle;
}
const instructions = $('[data-role="quiz-instructions"]');
const stage = $('[data-role="quiz-stage"]');
const expressionEl = $('[data-role="quiz-expression"]');
const statusEl = $('[data-role="quiz-status"]');
const streakEl = $('[data-role="quiz-streak"]');
const checkBtn = $('[data-role="quiz-check"]');
const nextBtn = $('[data-role="quiz-next"]');
const NEXT_PAGE = "9-dereferencing-ii.html";
const NEXT_LABEL = (() => {
    const label = getNavLabelForHref(NEXT_PAGE);
    return label ? `Next: ${label}` : "Next Program";
})();
const PULSE_CLASS = "pulse-success";
const TARGET_STREAK = 4;
const quiz = {
    streak: 0,
    passed: false,
    scenario: null,
    selected: null,
    awaitingNext: false,
    lastResult: null,
};
function randInt(min, max) {
    return Math.floor(Math.random() * (max - min + 1)) + min;
}
function pick(list) {
    return list[Math.floor(Math.random() * list.length)];
}
function shuffle(list) {
    const copy = [...list];
    for (let i = copy.length - 1; i > 0; i--) {
        const j = Math.floor(Math.random() * (i + 1));
        [copy[i], copy[j]] = [copy[j], copy[i]];
    }
    return copy;
}
function pointerDepth(type) {
    const matches = String(type || "").match(/\*/g);
    return matches ? matches.length : 0;
}
function buildBoxes(requiredDepth, minDepth = 1) {
    randAddr.reset(null);
    const maxDepth = randInt(Math.max(1, requiredDepth, minDepth), 3);
    const counts = {
        0: randInt(2, 4),
        1: randInt(1, 3),
        2: maxDepth >= 2 ? randInt(1, 2) : 0,
        3: maxDepth >= 3 ? randInt(1, 2) : 0,
    };
    if (maxDepth >= 2 && counts[1] < 1)
        counts[1] = 1;
    if (maxDepth >= 3 && counts[2] < 1)
        counts[2] = 1;
    const total = counts[0] + counts[1] + counts[2] + counts[3];
    const names = shuffle([
        "a",
        "b",
        "c",
        "d",
        "e",
        "f",
        "g",
        "h",
        "i",
        "j",
        "k",
        "m",
        "n",
        "p",
        "q",
        "r",
        "s",
        "t",
        "u",
        "v",
        "w",
        "x",
        "y",
        "z",
    ]).slice(0, total);
    const byDepth = {
        0: [],
        1: [],
        2: [],
        3: [],
    };
    let idx = 0;
    for (let i = 0; i < counts[0]; i++) {
        const name = names[idx++];
        byDepth[0].push({
            name,
            type: "int",
            value: String(randInt(1, 25)),
            address: "",
        });
    }
    for (let i = 0; i < counts[1]; i++) {
        const name = names[idx++];
        byDepth[1].push({
            name,
            type: "int*",
            value: "",
            address: "",
        });
    }
    for (let i = 0; i < counts[2]; i++) {
        const name = names[idx++];
        byDepth[2].push({
            name,
            type: "int**",
            value: "",
            address: "",
        });
    }
    for (let i = 0; i < counts[3]; i++) {
        const name = names[idx++];
        byDepth[3].push({
            name,
            type: "int***",
            value: "",
            address: "",
        });
    }
    const addressOrder = shuffle([
        ...byDepth[0],
        ...byDepth[1],
        ...byDepth[2],
        ...byDepth[3],
    ]);
    addressOrder.forEach((box) => {
        box.address = String(randAddr(box.type));
    });
    for (const ptr of byDepth[1]) {
        const target = pick(byDepth[0]);
        ptr.value = String(target.address);
    }
    for (const ptr of byDepth[2]) {
        const target = pick(byDepth[1]);
        ptr.value = String(target.address);
    }
    for (const ptr of byDepth[3]) {
        const target = pick(byDepth[2]);
        ptr.value = String(target.address);
    }
    return { boxes: addressOrder, maxDepth };
}
function derefTarget(boxMap, startBox, depth) {
    let current = startBox;
    for (let i = 0; i < depth; i++) {
        if (!current)
            return null;
        const next = boxMap.get(String(current.value));
        if (!next)
            return null;
        current = next;
    }
    return current;
}
function pickBoxQuestion(boxes, requiredDepth, minSourceDepth = 1) {
    const ptrs = boxes.filter((b) => pointerDepth(b.type) >= 1);
    const depth = Math.max(1, requiredDepth || 1);
    const minDepth = Math.max(depth, minSourceDepth);
    let eligible = ptrs.filter((b) => pointerDepth(b.type) >= minDepth);
    if (!eligible.length) {
        eligible = ptrs.filter((b) => pointerDepth(b.type) >= depth);
    }
    const source = eligible.length ? pick(eligible) : null;
    return { source, depth };
}
function pickSecondQuestionSource(boxes) {
    const ptrs = boxes.filter((b) => pointerDepth(b.type) >= 1);
    const byAddr = new Map(ptrs.map((b) => [String(b.address), b]));
    const pointedToByHigher = new Set();
    ptrs.forEach((b) => {
        const targetAddr = String(b.value || "");
        const target = byAddr.get(targetAddr);
        if (!target)
            return;
        const depth = pointerDepth(b.type);
        const targetDepth = pointerDepth(target.type);
        if (targetDepth === depth - 1) {
            pointedToByHigher.add(targetAddr);
        }
    });
    const eligible = ptrs.filter((b) => {
        const depth = pointerDepth(b.type);
        if (depth < 1 || depth > 2)
            return false;
        return pointedToByHigher.has(String(b.address));
    });
    return eligible.length ? pick(eligible) : null;
}
function requiredDepthFor(streak) {
    if (streak <= 1)
        return 1;
    if (streak === 2)
        return 2;
    return 3;
}
function buildScenario() {
    const requiredDepth = requiredDepthFor(quiz.streak);
    const needsPointedSource = quiz.streak <= 1;
    const minMaxDepth = needsPointedSource ? 2 : 1;
    const { boxes } = buildBoxes(requiredDepth, minMaxDepth);
    const byAddr = new Map(boxes.map((b) => [String(b.address), b]));
    let source = null;
    let depth = requiredDepth;
    if (needsPointedSource) {
        source = pickSecondQuestionSource(boxes);
        depth = 1;
    }
    else {
        ({ source, depth } = pickBoxQuestion(boxes, requiredDepth, 1));
    }
    if (!source)
        return buildScenario();
    const boxExpr = `${"*".repeat(depth)}${source.name}`;
    const target = derefTarget(byAddr, source, depth);
    if (!target)
        return buildScenario();
    return {
        boxes,
        boxExpr,
        boxTargetName: target ? target.name : "",
    };
}
function setSelected(name) {
    quiz.selected = name;
    stage?.querySelectorAll(".vbox").forEach((node) => {
        const el = node;
        el.classList.toggle("quiz-selected", el.dataset.name === name);
    });
}
function renderBoxes(boxes) {
    if (!stage)
        return;
    stage.innerHTML = "";
    boxes.forEach((b) => {
        const node = vbox({
            address: b.address ?? undefined,
            type: b.type,
            value: b.value,
            name: b.name,
            editable: false,
        });
        if ((b.value ?? "") === "")
            node.querySelector(".value")?.classList.add("placeholder", "muted");
        node.classList.add("quiz-selectable");
        node.dataset.name = b.name;
        node.addEventListener("click", () => setSelected(b.name));
        stage.appendChild(node);
    });
}
function updateStreak() {
    if (!streakEl)
        return;
    streakEl.textContent = `Streak: ${quiz.streak}/${TARGET_STREAK}`;
}
function setStatus(text, ok) {
    if (!statusEl)
        return;
    statusEl.textContent = text;
    statusEl.className = ok ? "ok" : "err";
    flashStatus(statusEl);
}
function setNextMode(mode) {
    if (!nextBtn)
        return;
    if (mode === "advance") {
        nextBtn.textContent = `${NEXT_LABEL} ▶▶`;
        return;
    }
    if (mode === "retry") {
        nextBtn.textContent = "Try Again";
        return;
    }
    nextBtn.textContent = "Next Question";
}
function setNextEnabled(enabled) {
    if (nextBtn)
        nextBtn.disabled = !enabled;
}
function clearNextPulse() {
    if (!nextBtn)
        return;
    nextBtn.classList.remove(PULSE_CLASS);
}
function pulseNext() {
    if (nextBtn)
        nextBtn.classList.add(PULSE_CLASS);
}
function clearSelection() {
    quiz.selected = null;
    stage?.querySelectorAll(".vbox").forEach((node) => {
        node.classList.remove("quiz-selected");
    });
}
function resetAttempt() {
    quiz.awaitingNext = false;
    quiz.lastResult = null;
    clearSelection();
    if (statusEl) {
        statusEl.textContent = "";
        statusEl.className = "muted";
    }
    if (checkBtn)
        checkBtn.disabled = false;
    setNextMode("next");
    setNextEnabled(false);
    clearNextPulse();
}
function setPrompt(scenario) {
    if (!instructions)
        return;
    const text = "Click on the variable that the expression refers to, then press $b{Check}.\nGet 4 in a row correct to pass.\nIn general, $c{*X} refers to the variable whose address is $n{X}'s value.";
    setPartsContent(instructions, text);
}
function newScenario() {
    quiz.scenario = buildScenario();
    resetAttempt();
    renderBoxes(quiz.scenario.boxes);
    setPrompt(quiz.scenario);
    if (expressionEl) {
        setPartsContent(expressionEl, `$c{${quiz.scenario.boxExpr}}`);
    }
    updateStreak();
}
function checkAnswer() {
    if (quiz.passed)
        return;
    if (quiz.awaitingNext)
        return;
    const scenario = quiz.scenario;
    const chosen = quiz.selected;
    if (!chosen) {
        setStatus("Select a box first.", false);
        return;
    }
    const boxOk = chosen === scenario?.boxTargetName;
    if (boxOk) {
        quiz.streak += 1;
        setStatus("correct", true);
        updateStreak();
        if (quiz.streak >= TARGET_STREAK) {
            quiz.passed = true;
            updateStreak();
            setNextMode("advance");
            setNextEnabled(true);
            if (checkBtn)
                checkBtn.disabled = true;
            quiz.awaitingNext = true;
            quiz.lastResult = "correct";
            pulseNext();
            return;
        }
        quiz.awaitingNext = true;
        quiz.lastResult = "correct";
        if (checkBtn)
            checkBtn.disabled = true;
        setNextMode("next");
        setNextEnabled(true);
        pulseNext();
        return;
    }
    quiz.streak = 0;
    setStatus("incorrect", false);
    updateStreak();
    quiz.awaitingNext = false;
    quiz.lastResult = "incorrect";
    if (checkBtn)
        checkBtn.disabled = false;
    setNextMode("next");
    setNextEnabled(false);
    clearNextPulse();
}
function sidebarParamValue() {
    return document.body.classList.contains("sidebar-collapsed") ? "0" : "1";
}
function withSidebarParam(url) {
    if (!url)
        return url;
    const [base, hash = ""] = url.split("#");
    const [path, query = ""] = base.split("?");
    const params = new URLSearchParams(query);
    params.set("sidebar", sidebarParamValue());
    const nextQuery = params.toString();
    const hashPart = hash ? `#${hash}` : "";
    return nextQuery ? `${path}?${nextQuery}${hashPart}` : `${path}${hashPart}`;
}
checkBtn?.addEventListener("click", checkAnswer);
nextBtn?.addEventListener("click", () => {
    clearNextPulse();
    if (!quiz.awaitingNext)
        return;
    if (quiz.passed) {
        window.location.href = withSidebarParam(NEXT_PAGE);
        return;
    }
    if (quiz.lastResult === "incorrect") {
        resetAttempt();
        return;
    }
    newScenario();
});
newScenario();
setNextMode("next");
setNextEnabled(false);
