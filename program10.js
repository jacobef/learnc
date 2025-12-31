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

  const instructions = $("#p10-instructions");
  const NEXT_PAGE = "sandbox.html";
  const FINISH_PARAM = "finished";

  const p10 = {
    lines: [
      "int x = 2;",
      "int y = 5;",
      "x = y + x;",
      "y = y + 1;",
      "x = x;",
      "y = y-y * 2;",
    ],
    boundary: 0,
    xAddr: randAddr("int"),
    yAddr: randAddr("int"),
    ws: Array(7).fill(null),
    passes: Array(7).fill(false),
    baseline: Array(7).fill(null),
  };

  const editableSteps = new Set([4, 5, 6]);

  const hint = createHintController({
    button: "#p10-hint-btn",
    panel: "#p10-hint",
    build: buildHint,
  });

  function resetHint() {
    hint.hide();
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
    if (!p10.baseline[boundary]) p10.baseline[boundary] = cloneBoxes(state);
    return p10.baseline[boundary];
  }

  function updateResetVisibility(boundary) {
    const resetBtn = $("#p10-reset");
    if (!resetBtn) return;
    const baseline = p10.baseline[boundary];
    const current = serializeWorkspace("p10workspace") || [];
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

  function setInstructions(message, { html = false } = {}) {
    if (!instructions) return;
    if (message && message.trim()) {
      if (html) instructions.innerHTML = message;
      else instructions.textContent = message;
      instructions.classList.remove("hidden");
    } else {
      instructions.textContent = "";
      instructions.classList.add("hidden");
    }
  }

  function updateInstructions() {
    if (p10.boundary === p10.lines.length && p10.passes[p10.lines.length]) {
      setInstructions("Program solved!");
      return;
    }
    if (p10.boundary === 0) {
      setInstructions(
        prependTopStepperNotice(
          "p10",
          "No instructions for this one. Good luck!",
          { html: true },
        ),
        { html: true },
      );
      return;
    }
    setInstructions("");
  }

  function renderCodePane10() {
    const progress = editableSteps.has(p10.boundary) && !p10.passes[p10.boundary];
    renderCodePane($("#p10-code"), p10.lines, p10.boundary, { progress });
  }

  function canonical(boundary) {
    const xAddr = String(p10.xAddr);
    const yAddr = String(p10.yAddr);
    const states = {
      1: [{ name: "x", type: "int", value: "2", address: xAddr }],
      2: [
        { name: "x", type: "int", value: "2", address: xAddr },
        { name: "y", type: "int", value: "5", address: yAddr },
      ],
      3: [
        { name: "x", type: "int", value: "7", address: xAddr },
        { name: "y", type: "int", value: "5", address: yAddr },
      ],
      4: [
        { name: "x", type: "int", value: "7", address: xAddr },
        { name: "y", type: "int", value: "6", address: yAddr },
      ],
      5: [
        { name: "x", type: "int", value: "7", address: xAddr },
        { name: "y", type: "int", value: "6", address: yAddr },
      ],
      6: [
        { name: "x", type: "int", value: "7", address: xAddr },
        { name: "y", type: "int", value: "-6", address: yAddr },
      ],
    };
    return cloneBoxes(states[boundary] || []);
  }

  function defaultsFor(boundary) {
    if (boundary <= 0) return [];
    if (editableSteps.has(boundary) && !p10.passes[boundary]) {
      return canonical(boundary - 1);
    }
    return canonical(boundary);
  }

  function render() {
    renderCodePane10();
    updateInstructions();
    const stage = $("#p10-stage");
    stage.innerHTML = "";
    if (editableSteps.has(p10.boundary) && p10.passes[p10.boundary]) {
      $("#p10-status").textContent = "correct";
      $("#p10-status").className = "ok";
    } else {
      $("#p10-status").textContent = "";
      $("#p10-status").className = "muted";
    }
    $("#p10-check").classList.add("hidden");
    $("#p10-reset").classList.add("hidden");
    $("#p10-add").classList.add("hidden");

    resetHint();

    if (p10.boundary > 0) {
      const editable =
        editableSteps.has(p10.boundary) && !p10.passes[p10.boundary];
      const defaults = defaultsFor(p10.boundary);
      const wrap = restoreWorkspace(
        p10.ws[p10.boundary],
        defaults,
        "p10workspace",
        {
          editable,
          deletable: editable,
        },
      );
      stage.appendChild(wrap);
      if (editable) {
        $("#p10-check").classList.remove("hidden");
        $("#p10-add").classList.remove("hidden");
        hint.setButtonHidden(false);
        ensureBaseline(p10.boundary, defaults);
        attachResetWatcher(wrap, p10.boundary);
      } else {
        p10.ws[p10.boundary] = cloneBoxes(defaults);
        hint.setButtonHidden(true);
        if (p10.boundary === p10.lines.length && editableSteps.size) {
          const solved = [...editableSteps].every((step) => p10.passes[step]);
          if (solved) p10.passes[p10.lines.length] = true;
        }
      }
    }
  }

  function save() {
    if (p10.boundary >= 1 && p10.boundary <= p10.lines.length) {
      p10.ws[p10.boundary] = serializeWorkspace("p10workspace");
    }
  }

  $("#p10-reset").onclick = () => {
    if (editableSteps.has(p10.boundary)) {
      p10.ws[p10.boundary] = null;
      render();
    }
  };

  $("#p10-add").onclick = () => {
    const ws = document.getElementById("p10workspace");
    if (!ws) return;
    ws.appendChild(makeAnswerBox({}));
    updateResetVisibility(p10.boundary);
  };

  $("#p10-hint-btn")?.addEventListener("click", () => {
    $("#p10-hint-btn")?.classList.remove("pulse-success");
  });

  $("#p10-check").onclick = () => {
    resetHint();
    if (!editableSteps.has(p10.boundary)) return;
    const ws = document.getElementById("p10workspace");
    if (!ws) return;
    const boxes = [...ws.querySelectorAll(".vbox")].map((v) => readBoxState(v));
    const by = Object.fromEntries(boxes.map((b) => [b.name, b]));
    const verdict = validateWorkspace(p10.boundary, boxes);
    $("#p10-status").textContent = verdict.ok ? "correct" : "incorrect";
    $("#p10-status").className = verdict.ok ? "ok" : "err";
    flashStatus($("#p10-status"));
    if (!verdict.ok && p10.boundary === 6 && by.y?.value === "0") {
      const hintBtn = $("#p10-hint-btn");
      if (hintBtn) {
        hintBtn.classList.remove("pulse-success");
        void hintBtn.offsetWidth;
        hintBtn.classList.add("pulse-success");
      }
    }
    if (verdict.ok) {
      p10.passes[p10.boundary] = true;
      p10.ws[p10.boundary] = boxes;
      ws.querySelectorAll(".vbox").forEach((v) => disableBoxEditing(v));
      removeBoxDeleteButtons(ws);
      $("#p10-check").classList.add("hidden");
      $("#p10-reset").classList.add("hidden");
      $("#p10-add").classList.add("hidden");
      hint.hide();
      $("#p10-hint-btn")?.classList.add("hidden");
      pulseNextButton("p10");
      updateInstructions();
      renderCodePane10();
      if (p10.boundary === p10.lines.length) {
        const solved = [...editableSteps].every((step) => p10.passes[step]);
        if (solved) p10.passes[p10.lines.length] = true;
      }
      pager.update();
    }
  };

  function expectedFor(boundary) {
    if (boundary === 4) {
      return [
        { name: "x", type: "int", value: "7" },
        { name: "y", type: "int", value: "6" },
      ];
    }
    if (boundary === 5) {
      return [
        { name: "x", type: "int", value: "7" },
        { name: "y", type: "int", value: "6" },
      ];
    }
    if (boundary === 6) {
      return [
        { name: "x", type: "int", value: "7" },
        { name: "y", type: "int", value: "-6" },
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
    if (!editableSteps.has(p10.boundary)) {
      return {
        html: 'Use <span class="btn-ref">Run line 1 ▶</span> to reach the editable lines.',
      };
    }
    if (p10.passes[p10.boundary]) return { html: "Looks good." };
    const ws = document.getElementById("p10workspace");
    if (!ws)
      return {
        html: 'Use <span class="btn-ref">Run line 1 ▶</span> to reach the editable lines.',
      };
    const boxes = [...ws.querySelectorAll(".vbox")].map((v) => readBoxState(v));
    if (!boxes.length)
      return {
        html: 'Use <span class="btn-ref">+ New variable</span> to add the variables you need.',
      };
    const by = Object.fromEntries(boxes.map((b) => [b.name, b]));
    if (!by.x || !by.y)
      return {
        html: 'You need <code class="tok-name">x</code> and <code class="tok-name">y</code> in the program state.',
      };
    if (by.x.type !== "int" || by.y.type !== "int")
      return {
        html: 'Both <code class="tok-name">x</code> and <code class="tok-name">y</code> should be <code class="tok-type">int</code>.',
      };
    if (p10.boundary === 6 && by.y.value === "0")
      return {
        html: "Haha, gotcha. Always remember order of operations.",
      };
    const verdict = validateWorkspace(p10.boundary, boxes);
    if (verdict.ok)
      return { html: 'Looks good. Press <span class="btn-ref">Check</span>.' };
    if (p10.boundary === 4) {
      if (by.x.value !== "7") {
        return {
          html: '<code class="tok-name">x</code> should stay as it was after line 3.',
        };
      }
      return {
        html: 'Line 4 increments <code class="tok-name">y</code>.',
      };
    }
    if (p10.boundary === 5) {
      return {
        html: 'Line 5 assigns <code class="tok-name">x</code> to itself, so nothing changes.',
      };
    }
    if (p10.boundary === 6) {
      return {
        html: 'Multiply before subtracting, and update <code class="tok-name">y</code> accordingly.',
      };
    }
    return "Your program has a problem that isn't covered by a hint. Sorry.";
  }

  const pager = createStepper({
    prefix: "p10",
    lines: p10.lines,
    nextPage: NEXT_PAGE,
    endLabel: "Finish",
    getBoundary: () => p10.boundary,
    setBoundary: (val) => {
      p10.boundary = val;
    },
    onBeforeChange: save,
    onAfterChange: () => {
      render();
    },
    isStepLocked: (boundary) => {
      if (editableSteps.has(boundary)) return !p10.passes[boundary];
      return false;
    },
    getStepBadge: (step) => {
      if (!editableSteps.has(step)) return "";
      return p10.passes[step] ? "check" : "note";
    },
  });

  $("#p10-next")?.addEventListener("click", (event) => {
    if (pager.boundary() !== p10.lines.length || !p10.passes[p10.lines.length])
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

  render();
  pager.update();
})(window.MB);
