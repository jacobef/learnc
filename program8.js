(function (MB) {
  const {
    $,
    randAddr,
    renderCodePane,
    readBoxState,
    isEmptyVal,
    restoreWorkspace,
    serializeWorkspace,
    makeAnswerBox,
    cloneBoxes,
    createHintController,
    createStepper,
    disableBoxEditing,
    removeBoxDeleteButtons,
    flashStatus,
    pulseNextButton,
    prependTopStepperNotice,
  } = MB;

  const instructions = $("#p8-instructions");
  const NEXT_PAGE = "program9.html";

  const p8 = {
    lines: [
      "int a; // mary had a",
      "// little lamb",
      "int b; int c;",
      "int d; // int e;",
      "int f",
      "= 5",
      ";",
      "/* whose fleece",
      "was white as snow */",
      "int g /* hi */ = 3;",
      "int",
      "h // = 9;",
      "/* = 10;",
      "= 11;",
      "*/ = 12;",
      "// = 13;",
    ],
    boundary: 0,
    addrs: {},
    ws: {},
    passes: { 4: false, 16: false },
    baseline: {},
  };

  const statementRanges = [
    { start: 4, end: 7, label: "Run lines 5-7" },
    { start: 10, end: 16, label: "Run lines ?-?" },
  ];

  const editableSteps = new Set([4, 16]);

  const hint = createHintController({
    button: "#p8-hint-btn",
    panel: "#p8-hint",
    build: buildHint,
  });

  function resetHint() {
    hint.hide();
  }

  function setInstructions(message, { html = false } = {}) {
    if (!instructions) return;
    if (message) {
      if (html) instructions.innerHTML = message;
      else instructions.textContent = message;
      instructions.classList.remove("hidden");
    } else {
      instructions.textContent = "";
      instructions.classList.add("hidden");
    }
  }

  function rangeStartingAt(boundary) {
    return statementRanges.find((range) => range.start === boundary) || null;
  }

  function rangeEndingAt(boundary) {
    return statementRanges.find((range) => range.end === boundary) || null;
  }

  function runLabelForBoundary(boundary) {
    const range = rangeStartingAt(boundary);
    if (range) return `${range.label} ▶`;
    return `Run line ${boundary + 1} ▶`;
  }

  function addr(name) {
    if (!p8.addrs[name]) p8.addrs[name] = randAddr("int");
    return p8.addrs[name];
  }

  function captureAddrs(boxes) {
    if (!Array.isArray(boxes)) return;
    boxes.forEach((box) => {
      const name = (box?.name || "").trim();
      if (!name) return;
      const addrValue = box.addr ?? box.address ?? null;
      if (addrValue != null) p8.addrs[name] = String(addrValue);
    });
  }

  function p8Save() {
    if (!editableSteps.has(p8.boundary)) return;
    const snap = serializeWorkspace("p8workspace");
    if (Array.isArray(snap) && snap.length) {
      p8.ws[p8.boundary] = snap;
      captureAddrs(snap);
    } else {
      p8.ws[p8.boundary] = null;
    }
  }

  function canonical(boundary) {
    const boxes = [];
    if (boundary >= 1) {
      boxes.push({
        address: String(addr("a")),
        type: "int",
        value: "empty",
        name: "a",
      });
    }
    if (boundary >= 3) {
      boxes.push(
        {
          address: String(addr("b")),
          type: "int",
          value: "empty",
          name: "b",
        },
        {
          address: String(addr("c")),
          type: "int",
          value: "empty",
          name: "c",
        },
      );
    }
    if (boundary >= 4) {
      boxes.push({
        address: String(addr("d")),
        type: "int",
        value: "empty",
        name: "d",
      });
    }
    if (boundary >= 7) {
      boxes.push({
        address: String(addr("f")),
        type: "int",
        value: "5",
        name: "f",
      });
    }
    if (boundary >= 10) {
      boxes.push({
        address: String(addr("g")),
        type: "int",
        value: "3",
        name: "g",
      });
    }
    if (boundary >= 16) {
      boxes.push({
        address: String(addr("h")),
        type: "int",
        value: "12",
        name: "h",
      });
    }
    return cloneBoxes(boxes);
  }

  function defaultsFor(boundary) {
    if (boundary <= 0) return [];
    if (editableSteps.has(boundary)) {
      const range = rangeEndingAt(boundary);
      if (range) return canonical(range.start);
      return canonical(boundary - 1);
    }
    return canonical(boundary);
  }

  function updateInstructions() {
    const runLabel = runLabelForBoundary(p8.boundary);
    if (p8.boundary === p8.lines.length && p8.passes[p8.lines.length]) {
      setInstructions("Program solved!");
      return;
    }
    if (p8.boundary === 0) {
      setInstructions(
        prependTopStepperNotice(
          "p8",
          `Click <span class="btn-ref">${runLabel}</span> to step through the program.`,
          { html: true },
        ),
        { html: true },
      );
    } else if (p8.boundary === 1) {
      setInstructions(
        'Anything after <code class="tok-line">//</code> on a line is ignored. This is called a comment.',
        { html: true },
      );
    } else if (p8.boundary === 2) {
      setInstructions("Comments can appear on their own lines as well.");
    } else if (p8.boundary === 3) {
      setInstructions("Multiple statements can appear on one line.");
    } else if (p8.boundary === 4 || p8.boundary === 16) {
      setInstructions("");
    } else if (p8.boundary === 7) {
      setInstructions("A statement can be split across multiple lines.");
    } else if (
      p8.boundary === 8 ||
      p8.boundary === 9 ||
      p8.boundary === 10 ||
      p8.boundary === 13 ||
      p8.boundary === 14
    ) {
      setInstructions(
        "A comment can appear on multiple lines, or within a line, beginning with <code class=\"tok-line\">/*</code> and ending with <code class=\"tok-line\">*/</code>.",
        { html: true },
      );
    } else {
      setInstructions("");
    }
  }

  function normalizeState(list) {
    if (!Array.isArray(list)) return [];
    return list
      .map((b) => ({
        name: (b.name || "").trim(),
        type: (b.type || "").trim(),
        value: String(b.value ?? "").trim(),
        address: String(b.addr ?? b.address ?? "").trim(),
      }))
      .sort((a, b) => a.name.localeCompare(b.name));
  }

  function statesEqual(a, b) {
    const na = normalizeState(a);
    const nb = normalizeState(b);
    if (na.length !== nb.length) return false;
    for (let i = 0; i < na.length; i++) {
      const left = na[i];
      const right = nb[i];
      if (left.name !== right.name) return false;
      if (left.type !== right.type) return false;
      if (left.value !== right.value) return false;
      if (left.address !== right.address) return false;
    }
    return true;
  }

  function ensureBaseline(boundary, state) {
    if (!p8.baseline[boundary]) p8.baseline[boundary] = cloneBoxes(state);
    return p8.baseline[boundary];
  }

  function updateResetVisibility(boundary) {
    const resetBtn = $("#p8-reset");
    if (!resetBtn) return;
    const baseline = p8.baseline[boundary];
    const current = serializeWorkspace("p8workspace") || [];
    const changed = Array.isArray(baseline) && !statesEqual(current, baseline);
    resetBtn.classList.toggle("hidden", !changed);
  }

  function attachResetWatcher(wrap, boundary) {
    if (!wrap) return;
    const refresh = () => updateResetVisibility(boundary);
    wrap.addEventListener("input", refresh);
    wrap.addEventListener("click", () => setTimeout(refresh, 0));
    refresh();
  }

  function expectedFor(boundary) {
    if (boundary === 4) {
      return [
        { name: "a", type: "int", value: "empty" },
        { name: "b", type: "int", value: "empty" },
        { name: "c", type: "int", value: "empty" },
        { name: "d", type: "int", value: "empty" },
      ];
    }
    if (boundary === 16) {
      return [
        { name: "a", type: "int", value: "empty" },
        { name: "b", type: "int", value: "empty" },
        { name: "c", type: "int", value: "empty" },
        { name: "d", type: "int", value: "empty" },
        { name: "f", type: "int", value: "5" },
        { name: "g", type: "int", value: "3" },
        { name: "h", type: "int", value: "12" },
      ];
    }
    return [];
  }

  function validateWorkspace(boundary, boxes) {
    const expected = expectedFor(boundary);
    if (!expected.length) return { ok: true };
    const cleaned = boxes.map((b) => ({
      name: (b.name || "").trim(),
      type: (b.type || "").trim(),
      value: String(b.value ?? "").trim(),
    }));
    const blank = cleaned.find((b) => !b.name);
    if (blank) {
      return {
        ok: false,
        message: {
          html: "Every box needs a name (delete the blank one if you don't need it).",
        },
      };
    }
    const counts = new Map();
    for (const box of cleaned) {
      counts.set(box.name, (counts.get(box.name) || 0) + 1);
      if (counts.get(box.name) > 1) {
        return {
          ok: false,
          message: {
            html: `There should only be one <code class="tok-name">${box.name}</code> box.`,
          },
        };
      }
    }
    const expectedNames = new Set(expected.map((e) => e.name));
    const extra = cleaned.find((b) => !expectedNames.has(b.name));
    if (extra) {
      return {
        ok: false,
        message: {
          html: `<code class="tok-name">${extra.name}</code> isn't part of the program state yet.`,
        },
      };
    }
    for (const need of expected) {
      const box = cleaned.find((b) => b.name === need.name);
      if (!box) {
        return {
          ok: false,
          message: {
            html: `Missing <code class="tok-name">${need.name}</code> in the program state.`,
          },
        };
      }
      if (box.type !== need.type) {
        return {
          ok: false,
          message: {
            html: `<code class="tok-name">${need.name}</code>'s type should be <code class="tok-type">${need.type}</code>.`,
          },
        };
      }
      if (need.value === "empty") {
        if (!isEmptyVal(box.value || "")) {
          return {
            ok: false,
            message: {
              html: `<code class="tok-name">${need.name}</code> should be blank.`,
            },
          };
        }
      } else if (isEmptyVal(box.value || "")) {
        return {
          ok: false,
          message: {
            html: `${need.name}'s value shouldn't be blank.`,
          },
        };
      } else if (box.value !== need.value) {
        return {
          ok: false,
          message: {
            html: `<code class="tok-name">${need.name}</code> should be <code class="tok-value">${need.value}</code>.`,
          },
        };
      }
    }
    return { ok: true };
  }

  function buildHint() {
    if (!editableSteps.has(p8.boundary)) {
      return {
        html: 'Use <span class="btn-ref">Run line 1 ▶</span> to reach the editable line.',
      };
    }
    if (p8.passes[p8.boundary]) return { html: "Looks good." };
    const ws = document.getElementById("p8workspace");
    if (!ws)
      return {
        html: 'Use <span class="btn-ref">Run line 1 ▶</span> to reach the editable line.',
      };
    const boxes = [...ws.querySelectorAll(".vbox")].map((v) =>
      readBoxState(v),
    );
    const escapeHint = (value) =>
      String(value)
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;")
        .replace(/'/g, "&#39;");
    if (
      p8.boundary === 4 &&
      boxes.some((box) => (box?.name || "").trim() === "e")
    ) {
      return { html: "<code class=\"tok-line\">// int e;</code> is a comment." };
    }
    if (p8.boundary === 16) {
      const h = boxes.find((box) => (box?.name || "").trim() === "h");
      const value = String(h?.value ?? "").trim();
      if (h && isEmptyVal(value)) {
        return { html: "h's value shouldn't be blank." };
      }
      if (value && !isEmptyVal(value) && value !== "12") {
        if (value === "9") {
          return {
            html: "<code class=\"tok-line\">// = 9;</code> is a comment.",
          };
        }
        if (value === "10" || value === "11") {
          return {
            html: "<code class=\"tok-line\">/* = 10;<br>= 11;<br>*/</code> is a comment.",
          };
        }
        if (value === "13") {
          return {
            html: "<code class=\"tok-line\">// = 13;</code> is a comment.",
          };
        }
        return `I'm not sure where you're getting ${escapeHint(value)} from.`;
      }
    }
    const verdict = validateWorkspace(p8.boundary, boxes);
    if (verdict.ok)
      return { html: 'Looks good. Press <span class="btn-ref">Check</span>.' };
    return verdict.message || "Keep going.";
  }

  function renderCodePane8() {
    const progress =
      editableSteps.has(p8.boundary) && !p8.passes[p8.boundary];
    let progressIndex;
    let progressRange;
    let doneBoundary;
    if (progress) {
      const range = rangeEndingAt(p8.boundary);
      if (range) {
        doneBoundary = range.start;
        progressIndex = range.start;
        progressRange = [range.start, p8.lines.length - 1];
      }
    }
    renderCodePane($("#p8-code"), p8.lines, p8.boundary, {
      progress,
      progressIndex,
      progressRange,
      doneBoundary,
    });
  }

  function p8Render() {
    renderCodePane8();
    updateInstructions();
    const stage = $("#p8-stage");
    stage.innerHTML = "";
    const addBtn = $("#p8-add");
    const checkBtn = $("#p8-check");
    const resetBtn = $("#p8-reset");
    const status = $("#p8-status");
    addBtn?.classList.add("hidden");
    checkBtn?.classList.add("hidden");
    resetBtn?.classList.add("hidden");
    hint.setButtonHidden(true);
    resetHint();

    if (editableSteps.has(p8.boundary) && p8.passes[p8.boundary]) {
      if (status) {
        status.textContent = "correct";
        status.className = "ok";
      }
    } else if (status) {
      status.textContent = "";
      status.className = "muted";
    }

    if (p8.boundary <= 0) return;
    const editable = editableSteps.has(p8.boundary) && !p8.passes[p8.boundary];
    const defaults = defaultsFor(p8.boundary);
    const wrap = restoreWorkspace(p8.ws[p8.boundary], defaults, "p8workspace", {
      editable,
      deletable: editable,
      allowNameEdit: editable,
      allowTypeEdit: editable,
    });
    stage.appendChild(wrap);
    if (editable) {
      addBtn?.classList.remove("hidden");
      checkBtn?.classList.remove("hidden");
      hint.setButtonHidden(false);
      ensureBaseline(p8.boundary, defaults);
      attachResetWatcher(wrap, p8.boundary);
    }
  }

  $("#p8-reset").onclick = () => {
    if (!editableSteps.has(p8.boundary)) return;
    p8.ws[p8.boundary] = null;
    p8.passes[p8.boundary] = false;
    p8.baseline[p8.boundary] = null;
    p8Render();
    pager.update();
  };

  $("#p8-check").onclick = () => {
    resetHint();
    if (!editableSteps.has(p8.boundary)) return;
    const ws = document.getElementById("p8workspace");
    if (!ws) return;
    const boxes = [...ws.querySelectorAll(".vbox")].map((v) =>
      readBoxState(v),
    );
    const verdict = validateWorkspace(p8.boundary, boxes);
    const status = $("#p8-status");
    if (status) {
      status.textContent = verdict.ok ? "correct" : "incorrect";
      status.className = verdict.ok ? "ok" : "err";
      flashStatus(status);
    }
    if (!verdict.ok) return;
    p8.passes[p8.boundary] = true;
    const snap = serializeWorkspace("p8workspace");
    if (Array.isArray(snap)) {
      p8.ws[p8.boundary] = snap;
      captureAddrs(snap);
    }
    ws.querySelectorAll(".vbox").forEach((v) => disableBoxEditing(v));
    removeBoxDeleteButtons(ws);
    $("#p8-check").classList.add("hidden");
    $("#p8-reset").classList.add("hidden");
    $("#p8-add").classList.add("hidden");
    hint.hide();
    $("#p8-hint-btn")?.classList.add("hidden");
    pulseNextButton("p8");
    p8Render();
    pager.update();
  };

  const pager = createStepper({
    prefix: "p8",
    lines: p8.lines,
    nextPage: NEXT_PAGE,
    endLabel: "Next Program",
    getBoundary: () => p8.boundary,
    setBoundary: (val) => {
      p8.boundary = val;
    },
    onBeforeChange: p8Save,
    onAfterChange: p8Render,
    isStepLocked: (boundary) =>
      editableSteps.has(boundary) && !p8.passes[boundary],
    getStepBadge: (step) => {
      if (step === 11) return p8.passes[16] ? "check" : "note";
      if (!editableSteps.has(step)) return "";
      return p8.passes[step] ? "check" : "note";
    },
    getNextLabel: (current) => {
      const range = rangeStartingAt(current);
      return range ? range.label : "";
    },
    getNextBoundary: (current) => {
      const range = rangeStartingAt(current);
      return range ? range.end : current + 1;
    },
    getPrevBoundary: (current) => {
      const range = rangeEndingAt(current);
      return range ? range.start : current - 1;
    },
  });

  $("#p8-add").onclick = () => {
    const ws = document.getElementById("p8workspace");
    if (!ws) return;
    ws.appendChild(makeAnswerBox({}));
    updateResetVisibility(p8.boundary);
  };

  p8Render();
  pager.update();
})(window.MB);
