(function (MB) {
  const {
    $,
    randAddr,
    renderCodePane,
    restoreWorkspace,
    serializeWorkspace,
    readBoxState,
    isEmptyVal,
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

  const instructions = $("#p9-instructions");
  const NEXT_PAGE = "program10.html";
  const FINISH_PARAM = "finished";

  const p9 = {
    lines: [
      "int a = 5 + 2 * (3 - 1);",
      "a = 1-3 * 4;",
      "a = (1 - 3) * 4;",
      "int b = 9 / 3;",
      "a = 5 / 3;",
      "a = -7 / 2;",
      "a = 1/2 + 1/2;",
      "a = 8 / -(2 + 1);",
      "int c = b+1 == 4;",
      "int d = b == 58;",
      "int e = 11/3 == 3;",
      "int f = 9 / 2+1 == 3;",
      "int g = 0 == 1 == 2;",
      "int h = (-2 / 3==1-1==1) - 3;",
    ],
    boundary: 0,
    addrs: {},
    ws: {},
    passes: { 2: false, 3: false, 8: false, 11: false, 12: false, 14: false },
    baseline: {},
    instructionsEnabled: true,
  };

  const statementRanges = [];

  const editableSteps = new Set([2, 3, 8, 11, 12, 14]);

  const hint = createHintController({
    button: "#p9-hint-btn",
    panel: "#p9-hint",
    build: buildHint,
  });

  function resetHint() {
    hint.hide();
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
    if (!p9.addrs[name]) p9.addrs[name] = randAddr("int");
    return p9.addrs[name];
  }

  function captureAddrs(boxes) {
    if (!Array.isArray(boxes)) return;
    boxes.forEach((box) => {
      const name = (box?.name || "").trim();
      if (!name) return;
      const addrValue = box.addr ?? box.address ?? null;
      if (addrValue != null) p9.addrs[name] = String(addrValue);
    });
  }

  function p9Save() {
    if (!editableSteps.has(p9.boundary)) return;
    const snap = serializeWorkspace("p9workspace");
    if (Array.isArray(snap) && snap.length) {
      p9.ws[p9.boundary] = snap;
      captureAddrs(snap);
    } else {
      p9.ws[p9.boundary] = null;
    }
  }

  function valueForA(boundary) {
    if (boundary >= 8) return "-2";
    if (boundary >= 7) return "0";
    if (boundary >= 6) return "-3";
    if (boundary >= 5) return "1";
    if (boundary >= 3) return "-8";
    if (boundary >= 2) return "-11";
    return "9";
  }

  function valueForC(boundary) {
    return "1";
  }

  function canonical(boundary) {
    const boxes = [];
    if (boundary >= 1) {
      boxes.push({
        address: String(addr("a")),
        type: "int",
        value: valueForA(boundary),
        name: "a",
      });
    }
    if (boundary >= 4) {
      boxes.push({
        address: String(addr("b")),
        type: "int",
        value: "3",
        name: "b",
      });
    }
    if (boundary >= 9) {
      boxes.push({
        address: String(addr("c")),
        type: "int",
        value: valueForC(boundary),
        name: "c",
      });
    }
    if (boundary >= 10) {
      boxes.push({
        address: String(addr("d")),
        type: "int",
        value: "0",
        name: "d",
      });
    }
    if (boundary >= 11) {
      boxes.push({
        address: String(addr("e")),
        type: "int",
        value: "1",
        name: "e",
      });
    }
    if (boundary >= 12) {
      boxes.push({
        address: String(addr("f")),
        type: "int",
        value: "0",
        name: "f",
      });
    }
    if (boundary >= 13) {
      boxes.push({
        address: String(addr("g")),
        type: "int",
        value: "0",
        name: "g",
      });
    }
    if (boundary >= 14) {
      boxes.push({
        address: String(addr("h")),
        type: "int",
        value: "-2",
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

  function updateInstructions() {
    if (!instructions) return;
    if (p9.boundary === p9.lines.length && p9.passes[p9.lines.length]) {
      setInstructions("Program solved!");
      return;
    }
    if (!p9.instructionsEnabled) {
      setInstructions("");
      return;
    }

    const editable = editableSteps.has(p9.boundary) && !p9.passes[p9.boundary];
    const runLabel = runLabelForBoundary(p9.boundary);
    if (p9.boundary === 0) {
      setInstructions(
        prependTopStepperNotice(
          "p9",
          'For a challenge, you can try this one without instructions. <button type="button" class="ub-explain-link" data-action="disable-instructions">Click here</button> if you\'d like to disable them. Otherwise, click <span class="btn-ref">Run line 1 ▶</span> to continue.',
          { html: true },
        ),
        { html: true },
      );
      return;
    }
    if (p9.boundary >= 1 && p9.boundary <= 3) {
      setInstructions(
        "PEMDAS order of operations applies, and isn't affected by spacing.",
      );
      return;
    }
    if (p9.boundary >= 5 && p9.boundary <= 8) {
      setInstructions(
        "Integer division drops the remainder, i.e. it rounds towards 0.",
      );
      return;
    }
    if (p9.boundary >= 9 && p9.boundary <= 12) {
      setInstructions(
        "x == y evaluates to 1 if x and y have equal values, and 0 if they don't. == has lower precedence than addition and subtraction.",
      );
      return;
    }
    if (p9.boundary === 13) {
      setInstructions("== is left-associative, so this is parsed as (0 == 1) == 2.");
      return;
    }
    if (p9.boundary === 14) {
      setInstructions("Remember that spacing doesn't affect order of operations.");
      return;
    }
    setInstructions("");
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
    if (!p9.baseline[boundary]) p9.baseline[boundary] = cloneBoxes(state);
    return p9.baseline[boundary];
  }

  function updateResetVisibility(boundary) {
    const resetBtn = $("#p9-reset");
    if (!resetBtn) return;
    const baseline = p9.baseline[boundary];
    const current = serializeWorkspace("p9workspace") || [];
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
    if (boundary === 2) {
      return [
        { name: "a", type: "int", value: "-11" },
      ];
    }
    if (boundary === 3) {
      return [
        { name: "a", type: "int", value: "-8" },
      ];
    }
    if (boundary === 8) {
      return [
        { name: "a", type: "int", value: "-2" },
        { name: "b", type: "int", value: "3" },
      ];
    }
    if (boundary === 11) {
      return [
        { name: "a", type: "int", value: "-2" },
        { name: "b", type: "int", value: "3" },
        { name: "c", type: "int", value: "1" },
        { name: "d", type: "int", value: "0" },
        { name: "e", type: "int", value: "1" },
      ];
    }
    if (boundary === 12) {
      return [
        { name: "a", type: "int", value: "-2" },
        { name: "b", type: "int", value: "3" },
        { name: "c", type: "int", value: "1" },
        { name: "d", type: "int", value: "0" },
        { name: "e", type: "int", value: "1" },
        { name: "f", type: "int", value: "0" },
      ];
    }
    if (boundary === 14) {
      return [
        { name: "a", type: "int", value: "-2" },
        { name: "b", type: "int", value: "3" },
        { name: "c", type: "int", value: "1" },
        { name: "d", type: "int", value: "0" },
        { name: "e", type: "int", value: "1" },
        { name: "f", type: "int", value: "0" },
        { name: "g", type: "int", value: "0" },
        { name: "h", type: "int", value: "-2" },
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
      if (isEmptyVal(box.value || "")) {
        return {
          ok: false,
          message: {
            html: `${need.name}'s value shouldn't be blank.`,
          },
        };
      }
      if (box.value !== need.value) {
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
    if (!editableSteps.has(p9.boundary)) {
      return {
        html: 'Use <span class="btn-ref">Run line 1 ▶</span> to reach the editable lines.',
      };
    }
    if (p9.passes[p9.boundary]) return { html: "Looks good." };
    const ws = document.getElementById("p9workspace");
    if (!ws)
      return {
        html: 'Use <span class="btn-ref">Run line 1 ▶</span> to reach the editable lines.',
      };
    const boxes = [...ws.querySelectorAll(".vbox")].map((v) =>
      readBoxState(v),
    );
    if (!boxes.length)
      return {
        html: 'Use <span class="btn-ref">+ New variable</span> to add the variables you need.',
      };
    const by = Object.fromEntries(boxes.map((b) => [b.name, b]));
    if (p9.boundary === 2) {
      if (!by.a)
        return {
          html: 'Keep <code class="tok-name">a</code> from line 1 in the program state.',
        };
      if (by.a.type !== "int")
        return {
          html: '<code class="tok-name">a</code>\'s type should be <code class="tok-type">int</code>.',
        };
      if (isEmptyVal(by.a.value || ""))
        return {
          html: 'Line 2 is <code class="tok-line">1 - (3 * 4)</code>.',
        };
      if (by.a.value !== "-11")
        return {
          html: 'Multiply before subtracting, then update <code class="tok-name">a</code>.',
        };
    }
    if (p9.boundary === 3) {
      if (!by.a)
        return {
          html: 'Keep <code class="tok-name">a</code> in the program state.',
        };
      if (by.a.type !== "int")
        return {
          html: '<code class="tok-name">a</code>\'s type should be <code class="tok-type">int</code>.',
        };
      if (isEmptyVal(by.a.value || ""))
        return {
          html: 'Line 3 is <code class="tok-line">(1 - 3) * 4</code>.',
        };
      if (by.a.value !== "-8")
        return {
          html: 'Evaluate the parentheses first, then multiply, then update <code class="tok-name">a</code>.',
        };
    }
    if (p9.boundary === 8) {
      if (!by.a)
        return {
          html: 'Keep <code class="tok-name">a</code> in the program state.',
        };
      if (by.a.type !== "int")
        return {
          html: '<code class="tok-name">a</code>\'s type should be <code class="tok-type">int</code>.',
        };
      if (isEmptyVal(by.a.value || ""))
        return {
          html: 'Line 8 is <code class="tok-line">8 / -(2 + 1)</code>.',
        };
      if (by.a.value !== "-2")
        return {
          html: 'Compute the parentheses first, then divide, then update <code class="tok-name">a</code>.',
        };
    }
    if (p9.boundary === 11) {
      if (!by.e)
        return {
          html: 'Add <code class="tok-name">e</code> for line 11.',
        };
      if (by.e.type !== "int")
        return {
          html: '<code class="tok-name">e</code>\'s type should be <code class="tok-type">int</code>.',
        };
      if (isEmptyVal(by.e.value || ""))
        return {
          html: 'Line 11 is <code class="tok-line">11 / 3 == 3</code>.',
        };
      if (by.e.value !== "1")
        return {
          html: 'Do the division first, then decide whether the comparison is true or false.',
        };
    }
    if (p9.boundary === 12) {
      if (!by.f)
        return {
          html: 'Add <code class="tok-name">f</code> for line 12.',
        };
      if (isEmptyVal(by.f.value || ""))
        return {
          html: 'Line 12 is <code class="tok-line">9 / 2 + 1 == 3</code>.',
        };
      if (by.f.value !== "0")
        return {
          html: 'Evaluate division and addition first, then decide whether the comparison is true or false.',
        };
    }
    if (p9.boundary === 14) {
      if (!by.g)
        return {
          html: 'Keep <code class="tok-name">g</code> from line 13 in the program state.',
        };
      if (!by.h)
        return {
          html: 'Add <code class="tok-name">h</code> for line 14.',
        };
      if (isEmptyVal(by.h.value || ""))
        return {
          html: 'This line should be parsed as <code class="tok-line">((( -2 / 3 ) == (1 - 1)) == 1) - 3</code>.',
        };
      if (by.h.value !== "-2")
        return {
          html: 'Work from the innermost parentheses outward, and remember comparisons yield 0 or 1.',
        };
    }
    const verdict = validateWorkspace(p9.boundary, boxes);
    if (verdict.ok)
      return { html: 'Looks good. Press <span class="btn-ref">Check</span>.' };
    return {
      html: "Something is off. Double-check variable names, types, and order of operations.",
    };
  }

  function renderCodePane9() {
    const progress = editableSteps.has(p9.boundary) && !p9.passes[p9.boundary];
    let progressIndex;
    let progressRange;
    let doneBoundary;
    if (progress) {
      const range = rangeEndingAt(p9.boundary);
      if (range) {
        doneBoundary = range.start;
        progressIndex = range.start;
        progressRange = [range.start, range.end - 1];
      }
    }
    renderCodePane($("#p9-code"), p9.lines, p9.boundary, {
      progress,
      progressIndex,
      progressRange,
      doneBoundary,
    });
  }

  function p9Render() {
    renderCodePane9();
    updateInstructions();
    const stage = $("#p9-stage");
    stage.innerHTML = "";
    const addBtn = $("#p9-add");
    const checkBtn = $("#p9-check");
    const resetBtn = $("#p9-reset");
    const status = $("#p9-status");
    addBtn?.classList.add("hidden");
    checkBtn?.classList.add("hidden");
    resetBtn?.classList.add("hidden");
    hint.setButtonHidden(true);
    resetHint();

    if (editableSteps.has(p9.boundary) && p9.passes[p9.boundary]) {
      if (status) {
        status.textContent = "correct";
        status.className = "ok";
      }
    } else if (status) {
      status.textContent = "";
      status.className = "muted";
    }

    if (p9.boundary <= 0) return;
    const editable = editableSteps.has(p9.boundary) && !p9.passes[p9.boundary];
    const defaults = defaultsFor(p9.boundary);
    const wrap = restoreWorkspace(p9.ws[p9.boundary], defaults, "p9workspace", {
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
      ensureBaseline(p9.boundary, defaults);
      attachResetWatcher(wrap, p9.boundary);
    }
  }

  $("#p9-reset").onclick = () => {
    if (!editableSteps.has(p9.boundary)) return;
    p9.ws[p9.boundary] = null;
    p9.passes[p9.boundary] = false;
    p9.baseline[p9.boundary] = null;
    p9Render();
    pager.update();
  };

  $("#p9-check").onclick = () => {
    resetHint();
    if (!editableSteps.has(p9.boundary)) return;
    const ws = document.getElementById("p9workspace");
    if (!ws) return;
    const boxes = [...ws.querySelectorAll(".vbox")].map((v) =>
      readBoxState(v),
    );
    const verdict = validateWorkspace(p9.boundary, boxes);
    const status = $("#p9-status");
    if (status) {
      status.textContent = verdict.ok ? "correct" : "incorrect";
      status.className = verdict.ok ? "ok" : "err";
      flashStatus(status);
    }
    if (!verdict.ok) return;
    p9.passes[p9.boundary] = true;
    const snap = serializeWorkspace("p9workspace");
    if (Array.isArray(snap)) {
      p9.ws[p9.boundary] = snap;
      captureAddrs(snap);
    }
    ws.querySelectorAll(".vbox").forEach((v) => disableBoxEditing(v));
    removeBoxDeleteButtons(ws);
    $("#p9-check").classList.add("hidden");
    $("#p9-reset").classList.add("hidden");
    $("#p9-add").classList.add("hidden");
    hint.hide();
    $("#p9-hint-btn")?.classList.add("hidden");
    pulseNextButton("p9");
    p9Render();
    pager.update();
  };

  const pager = createStepper({
    prefix: "p9",
    lines: p9.lines,
    nextPage: NEXT_PAGE,
    endLabel: "Next Program",
    getBoundary: () => p9.boundary,
    setBoundary: (val) => {
      p9.boundary = val;
    },
    onBeforeChange: p9Save,
    onAfterChange: p9Render,
    isStepLocked: (boundary) =>
      editableSteps.has(boundary) && !p9.passes[boundary],
    getStepBadge: (step) => {
      if (!editableSteps.has(step)) return "";
      return p9.passes[step] ? "check" : "note";
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

  $("#p9-add").onclick = () => {
    const ws = document.getElementById("p9workspace");
    if (!ws) return;
    ws.appendChild(makeAnswerBox({}));
    updateResetVisibility(p9.boundary);
  };

  instructions?.addEventListener("click", (event) => {
    const target = event.target;
    if (!(target instanceof Element)) return;
    if (!target.matches("[data-action='disable-instructions']")) return;
    event.preventDefault();
    p9.instructionsEnabled = false;
    updateInstructions();
  });

  $("#p9-next")?.addEventListener("click", (event) => {
    if (pager.boundary() !== p9.lines.length || !p9.passes[p9.lines.length])
      return;
    const url = new URL(NEXT_PAGE, window.location.href);
    const sidebar = document.body.classList.contains("sidebar-collapsed")
      ? "0"
      : "1";
    url.searchParams.set(FINISH_PARAM, "1");
    url.searchParams.set("sidebar", sidebar);
    event.preventDefault();
    event.stopImmediatePropagation();
    window.location.href = url.toString();
  });

  p9Render();
  pager.update();
})(window.MB);
