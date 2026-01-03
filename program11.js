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

  const instructions = $("#p11-instructions");
  const NEXT_PAGE = "program12.html";

  const p11 = {
    lines: [
      "int yin = 2;",
      "int yang = 5;",
      "yin = yang + yin;",
      "yang = yang + 1;",
      "yin = yin;",
      "yang = yang-yang * 2;",
    ],
    boundary: 0,
    yinAddr: randAddr("int"),
    yangAddr: randAddr("int"),
    ws: Array(7).fill(null),
    passes: Array(7).fill(false),
    baseline: Array(7).fill(null),
  };

  const editableSteps = new Set([4, 5, 6]);

  const hint = createHintController({
    button: "#p11-hint-btn",
    panel: "#p11-hint",
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
    if (!p11.baseline[boundary]) p11.baseline[boundary] = cloneBoxes(state);
    return p11.baseline[boundary];
  }

  function updateResetVisibility(boundary) {
    const resetBtn = $("#p11-reset");
    if (!resetBtn) return;
    const baseline = p11.baseline[boundary];
    const current = serializeWorkspace("p11workspace") || [];
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
    if (p11.boundary === p11.lines.length && p11.passes[p11.lines.length]) {
      setInstructions("Program solved!");
      return;
    }
    if (p11.boundary === 0) {
      setInstructions(
        prependTopStepperNotice(
          "p11",
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
    const progress = editableSteps.has(p11.boundary) && !p11.passes[p11.boundary];
    renderCodePane($("#p11-code"), p11.lines, p11.boundary, { progress });
  }

  function canonical(boundary) {
    const yinAddr = String(p11.yinAddr);
    const yangAddr = String(p11.yangAddr);
    const states = {
      1: [{ name: "yin", type: "int", value: "2", address: yinAddr }],
      2: [
        { name: "yin", type: "int", value: "2", address: yinAddr },
        { name: "yang", type: "int", value: "5", address: yangAddr },
      ],
      3: [
        { name: "yin", type: "int", value: "7", address: yinAddr },
        { name: "yang", type: "int", value: "5", address: yangAddr },
      ],
      4: [
        { name: "yin", type: "int", value: "7", address: yinAddr },
        { name: "yang", type: "int", value: "6", address: yangAddr },
      ],
      5: [
        { name: "yin", type: "int", value: "7", address: yinAddr },
        { name: "yang", type: "int", value: "6", address: yangAddr },
      ],
      6: [
        { name: "yin", type: "int", value: "7", address: yinAddr },
        { name: "yang", type: "int", value: "-6", address: yangAddr },
      ],
    };
    return cloneBoxes(states[boundary] || []);
  }

  function defaultsFor(boundary) {
    if (boundary <= 0) return [];
    if (editableSteps.has(boundary) && !p11.passes[boundary]) {
      return canonical(boundary - 1);
    }
    return canonical(boundary);
  }

  function render() {
    renderCodePane10();
    updateInstructions();
    const stage = $("#p11-stage");
    stage.innerHTML = "";
    if (editableSteps.has(p11.boundary) && p11.passes[p11.boundary]) {
      $("#p11-status").textContent = "correct";
      $("#p11-status").className = "ok";
    } else {
      $("#p11-status").textContent = "";
      $("#p11-status").className = "muted";
    }
    $("#p11-check").classList.add("hidden");
    $("#p11-reset").classList.add("hidden");
    $("#p11-add").classList.add("hidden");

    resetHint();

    if (p11.boundary > 0) {
      const editable =
        editableSteps.has(p11.boundary) && !p11.passes[p11.boundary];
      const defaults = defaultsFor(p11.boundary);
      const wrap = restoreWorkspace(
        p11.ws[p11.boundary],
        defaults,
        "p11workspace",
        {
          editable,
          deletable: editable,
        },
      );
      stage.appendChild(wrap);
      if (editable) {
        $("#p11-check").classList.remove("hidden");
        $("#p11-add").classList.remove("hidden");
        hint.setButtonHidden(false);
        ensureBaseline(p11.boundary, defaults);
        attachResetWatcher(wrap, p11.boundary);
        wrap.addEventListener("input", () => {
          $("#p11-hint-btn")?.classList.remove("pulse-success");
        });
      } else {
        p11.ws[p11.boundary] = cloneBoxes(defaults);
        hint.setButtonHidden(true);
        if (p11.boundary === p11.lines.length && editableSteps.size) {
          const solved = [...editableSteps].every((step) => p11.passes[step]);
          if (solved) p11.passes[p11.lines.length] = true;
        }
      }
    }
  }

  function save() {
    if (p11.boundary >= 1 && p11.boundary <= p11.lines.length) {
      p11.ws[p11.boundary] = serializeWorkspace("p11workspace");
    }
  }

  $("#p11-reset").onclick = () => {
    if (editableSteps.has(p11.boundary)) {
      p11.ws[p11.boundary] = null;
      render();
    }
  };

  $("#p11-add").onclick = () => {
    const ws = document.getElementById("p11workspace");
    if (!ws) return;
    ws.appendChild(makeAnswerBox({}));
    updateResetVisibility(p11.boundary);
  };

  $("#p11-hint-btn")?.addEventListener("click", () => {
    $("#p11-hint-btn")?.classList.remove("pulse-success");
  });

  $("#p11-check").onclick = () => {
    resetHint();
    if (!editableSteps.has(p11.boundary)) return;
    const ws = document.getElementById("p11workspace");
    if (!ws) return;
    const boxes = [...ws.querySelectorAll(".vbox")].map((v) => readBoxState(v));
    const by = Object.fromEntries(boxes.map((b) => [b.name, b]));
    const verdict = validateWorkspace(p11.boundary, boxes);
    $("#p11-status").textContent = verdict.ok ? "correct" : "incorrect";
    $("#p11-status").className = verdict.ok ? "ok" : "err";
    flashStatus($("#p11-status"));
    if (!verdict.ok && p11.boundary === 6 && by.yang?.value === "0") {
      const hintBtn = $("#p11-hint-btn");
      if (hintBtn) {
        hintBtn.classList.remove("pulse-success");
        void hintBtn.offsetWidth;
        hintBtn.classList.add("pulse-success");
      }
    }
    if (verdict.ok) {
      p11.passes[p11.boundary] = true;
      p11.ws[p11.boundary] = boxes;
      ws.querySelectorAll(".vbox").forEach((v) => disableBoxEditing(v));
      removeBoxDeleteButtons(ws);
      $("#p11-check").classList.add("hidden");
      $("#p11-reset").classList.add("hidden");
      $("#p11-add").classList.add("hidden");
      hint.hide();
      $("#p11-hint-btn")?.classList.add("hidden");
      pulseNextButton("p11");
      updateInstructions();
      renderCodePane10();
      if (p11.boundary === p11.lines.length) {
        const solved = [...editableSteps].every((step) => p11.passes[step]);
        if (solved) p11.passes[p11.lines.length] = true;
      }
      pager.update();
    }
  };

  function expectedFor(boundary) {
    if (boundary === 4) {
      return [
        { name: "yin", type: "int", value: "7" },
        { name: "yang", type: "int", value: "6" },
      ];
    }
    if (boundary === 5) {
      return [
        { name: "yin", type: "int", value: "7" },
        { name: "yang", type: "int", value: "6" },
      ];
    }
    if (boundary === 6) {
      return [
        { name: "yin", type: "int", value: "7" },
        { name: "yang", type: "int", value: "-6" },
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
    if (!editableSteps.has(p11.boundary)) {
      return {
        html: 'Use <span class="btn-ref">Run line 1 ▶</span> to reach the editable lines.',
      };
    }
    if (p11.passes[p11.boundary]) return { html: "Looks good." };
    const ws = document.getElementById("p11workspace");
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
    if (!by.yin || !by.yang)
      return {
        html: 'You need <code class="tok-name">yin</code> and <code class="tok-name">yang</code> in the program state.',
      };
    if (by.yin.type !== "int" || by.yang.type !== "int")
      return {
        html: 'Both <code class="tok-name">yin</code> and <code class="tok-name">yang</code> should be <code class="tok-type">int</code>.',
      };
    if (p11.boundary === 6 && by.yang.value === "0")
      return {
        html: "Haha, gotcha. Always remember order of operations.",
      };
    const verdict = validateWorkspace(p11.boundary, boxes);
    if (verdict.ok)
      return { html: 'Looks good. Press <span class="btn-ref">Check</span>.' };
    if (p11.boundary === 4) {
      if (by.yin.value !== "7") {
        return {
          html: '<code class="tok-name">yin</code> should stay as it was after line 3.',
        };
      }
      return {
        html: 'Line 4 increments <code class="tok-name">yang</code>.',
      };
    }
    if (p11.boundary === 5) {
      return {
        html: 'Line 5 assigns <code class="tok-name">yin</code> to itself, so nothing changes.',
      };
    }
    if (p11.boundary === 6) {
      return {
        html: 'Multiply before subtracting, and update <code class="tok-name">yang</code> accordingly.',
      };
    }
    return "Your program has a problem that isn't covered by a hint. Sorry.";
  }

  const pager = createStepper({
    prefix: "p11",
    lines: p11.lines,
    nextPage: NEXT_PAGE,
    endLabel: "Next Program",
    getBoundary: () => p11.boundary,
    setBoundary: (val) => {
      p11.boundary = val;
    },
    onBeforeChange: save,
    onAfterChange: () => {
      render();
    },
    isStepLocked: (boundary) => {
      if (editableSteps.has(boundary)) return !p11.passes[boundary];
      return false;
    },
    getStepBadge: (step) => {
      if (!editableSteps.has(step)) return "";
      return p11.passes[step] ? "check" : "note";
    },
  });

  render();
  pager.update();
})(window.MB);
