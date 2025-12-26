(function (MB) {
  const {
    $,
    renderCodePane,
    readBoxState,
    restoreWorkspace,
    serializeWorkspace,
    randAddr,
    cloneBoxes,
    isEmptyVal,
    createHintController,
    createStepper,
    makeAnswerBox,
  } = MB;

  const instructions = $("#p7a-instructions");
  const NEXT_PAGE = "program7b.html";

  const p7a = {
    lines: [
      "int deer;",
      "int hare;",
      "int* wolf;",
      "wolf = &deer;",
      "wolf = &hare;",
      "int** bear = &wolf;",
      "int* fox = wolf;",
    ],
    boundary: 0,
    aAddr: randAddr("int"),
    bAddr: randAddr("int"),
    ptrAddr: randAddr("int*"),
    pptrAddr: randAddr("int**"),
    spareAddr: randAddr("int*"),
    ws: Array(8).fill(null),
    passes: Array(8).fill(false),
    baseline: Array(8).fill(null),
  };

  const editableSteps = new Set([5, 6, 7]);

  function ptrTarget(boundary) {
    if (boundary < 4) return "empty";
    if (boundary < 5) return String(p7a.aAddr);
    return String(p7a.bAddr);
  }

  function pptrTarget(boundary) {
    if (boundary < 6) return "empty";
    return String(p7a.ptrAddr);
  }

  function canonical(boundary) {
    const state = [];
    if (boundary >= 1) {
      state.push({
        name: "deer",
        names: ["deer"],
        type: "int",
        value: "empty",
        address: String(p7a.aAddr),
      });
    }
    if (boundary >= 2) {
      state.push({
        name: "hare",
        names: ["hare"],
        type: "int",
        value: "empty",
        address: String(p7a.bAddr),
      });
    }
    if (boundary >= 3) {
      state.push({
        name: "wolf",
        names: ["wolf"],
        type: "int*",
        value: ptrTarget(boundary),
        address: String(p7a.ptrAddr),
      });
    }
    if (boundary >= 6) {
      state.push({
        name: "bear",
        names: ["bear"],
        type: "int**",
        value: pptrTarget(boundary),
        address: String(p7a.pptrAddr),
      });
    }
    if (boundary >= 7) {
      state.push({
        name: "fox",
        names: ["fox"],
        type: "int*",
        value: String(p7a.bAddr),
        address: String(p7a.spareAddr),
      });
    }
    return state;
  }

  function carriedState(boundary) {
    for (let b = boundary - 1; b >= 0; b--) {
      const st = p7a.ws[b];
      if (Array.isArray(st) && st.length) return cloneBoxes(st);
    }
    return null;
  }

  function defaultsFor(boundary) {
    if (!editableSteps.has(boundary)) return canonical(boundary);
    const carried = carriedState(boundary);
    const base = carried
      ? cloneBoxes(carried)
      : cloneBoxes(canonical(boundary - 1));
    return base;
  }

  function setInstructions(message) {
    if (!instructions) return;
    if (message && message.trim()) {
      instructions.textContent = message;
      instructions.classList.remove("hidden");
    } else {
      instructions.textContent = "";
      instructions.classList.add("hidden");
    }
  }

  function updateStatus() {
    if (p7a.boundary === 0) {
      setInstructions("No instructions for this one. Good luck!");
    } else {
      setInstructions("");
    }
  }

  const hint = createHintController({
    button: "#p7a-hint-btn",
    panel: "#p7a-hint",
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
    if (!p7a.baseline[boundary]) p7a.baseline[boundary] = cloneBoxes(state);
    return p7a.baseline[boundary];
  }

  function updateResetVisibility(boundary) {
    const resetBtn = $("#p7a-reset");
    if (!resetBtn) return;
    const baseline = p7a.baseline[boundary];
    const current = serializeWorkspace("p7aworkspace") || [];
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

  function buildHint() {
    const ws = document.getElementById("p7aworkspace");
    if (!ws)
      return {
        html: 'Use <span class="btn-ref">Run line 1 ▶</span> to reach the editable line.',
      };
    const boxes = [...ws.querySelectorAll(".vbox")].map((v) => readBoxState(v));
    const by = Object.fromEntries(boxes.map((b) => [b.name, b]));
    if (!by.deer || !by.hare || !by.wolf)
      return {
        html: 'You still need <code class="tok-name">deer</code>, <code class="tok-name">hare</code>, and <code class="tok-name">wolf</code> in the program state.',
      };
    if (p7a.boundary >= 6 && !by.bear)
      return {
        html: 'You need <code class="tok-name">bear</code> in the program state.',
      };
    if (by.deer.type !== "int" || by.hare.type !== "int")
      return {
        html: 'The types of <code class="tok-name">deer</code> and <code class="tok-name">hare</code> should be <code class="tok-type">int</code>.',
      };
    if (by.wolf.type !== "int*")
      return {
        html: '<code class="tok-name">wolf</code>\'s type should be <code class="tok-type">int*</code>.',
      };
    if (p7a.boundary >= 6 && by.bear && by.bear.type !== "int**")
      return {
        html: '<code class="tok-name">bear</code>\'s type should be <code class="tok-type">int**</code>.',
      };
    if (p7a.boundary >= 7 && by.fox && by.fox.type !== "int*")
      return {
        html: '<code class="tok-name">fox</code>\'s type should be <code class="tok-type">int*</code>.',
      };
    // Skip address validation; addresses are generated for the user.
    const ptrVal = (by.wolf.value || "").trim();
    if (p7a.boundary < 4) {
      if (!isEmptyVal(ptrVal))
        return {
          html: '<code class="tok-name">wolf</code> was just declared, so its value should be empty.',
        };
    } else {
      if (!ptrVal)
        return {
          html: '<code class="tok-name">wolf</code>\'s value should not be empty.',
        };
      if (ptrVal !== String(ptrTarget(p7a.boundary))) {
        const target =
          p7a.boundary < 5 ? "deer" : "hare";
        return {
          html: `This line sets <code class="tok-name">wolf</code>'s value to <code class="tok-name">${target}</code>'s address.`,
        };
      }
    }
    if (p7a.boundary === 6) {
      const pptrVal = (by.bear?.value || "").trim();
      if (!pptrVal)
        return {
          html: 'Set <code class="tok-name">bear</code> to store <code class="tok-name">wolf</code>\'s address.',
        };
      if (pptrVal !== String(p7a.ptrAddr))
        return {
          html: '<code class="tok-name">bear</code> should hold <code class="tok-name">wolf</code>\'s address.',
        };
    }
    if (p7a.boundary === 7) {
      if (!by.fox)
        return {
          html: 'You need <code class="tok-name">fox</code> in the program state.',
        };
      const spareVal = (by.fox.value || "").trim();
      if (spareVal === String(p7a.ptrAddr))
        return {
          html: '<code class="tok-name">fox</code> is being assigned to <code class="tok-name">wolf</code>, not <code class="tok-addr">&amp;wolf</code>, so it should be set to <code class="tok-name">wolf</code>\'s value, not <code class="tok-name">wolf</code>\'s address.',
        };
      if (spareVal !== String(p7a.bAddr))
        return {
          html: '<code class="tok-name">fox</code>\'s value should be set to <code class="tok-name">wolf</code>\'s value.',
        };
    }

    if (isStepCorrect(boxes))
      return { html: 'Looks good. Press <span class="btn-ref">Check</span>.' };
    const hasReset = !!document.getElementById("p7a-reset");
    return hasReset
      ? {
          html: 'Your program has a problem that isn\'t covered by a hint. Try starting over on this step by clicking <span class="btn-ref">Reset</span>.',
        }
      : "Your program has a problem that isn't covered by a hint. Sorry.";
  }

  function renderCode() {
    const progress =
      editableSteps.has(p7a.boundary) && !p7a.passes[p7a.boundary];
    renderCodePane($("#p7a-code"), p7a.lines, p7a.boundary, { progress });
  }

  function render() {
    renderCode();
    const stage = $("#p7a-stage");
    stage.innerHTML = "";
    if (editableSteps.has(p7a.boundary) && p7a.passes[p7a.boundary]) {
      $("#p7a-status").textContent = "correct";
      $("#p7a-status").className = "ok";
    } else {
      $("#p7a-status").textContent = "";
      $("#p7a-status").className = "muted";
    }
    $("#p7a-check").classList.add("hidden");
    $("#p7a-reset").classList.add("hidden");
    $("#p7a-add").classList.add("hidden");
    hint.setButtonHidden(true);
    resetHint();

    if (p7a.boundary > 0) {
      const editable =
        editableSteps.has(p7a.boundary) && !p7a.passes[p7a.boundary];
      const defaults = defaultsFor(p7a.boundary);
      const wrap = restoreWorkspace(
        p7a.ws[p7a.boundary],
        defaults,
        "p7aworkspace",
        {
          editable,
          deletable: editable,
          allowNameAdd: false,
          allowNameEdit: false,
          allowTypeEdit: false,
        },
      );
      stage.appendChild(wrap);
      if (editable) {
        $("#p7a-check").classList.remove("hidden");
        $("#p7a-add").classList.remove("hidden");
        hint.setButtonHidden(false);
        ensureBaseline(p7a.boundary, defaults);
        attachResetWatcher(wrap, p7a.boundary);
      } else {
        p7a.ws[p7a.boundary] = cloneBoxes(defaults);
        p7a.passes[p7a.boundary] = true;
      }
    }
  }

  function save() {
    if (p7a.boundary >= 1 && p7a.boundary <= p7a.lines.length) {
      p7a.ws[p7a.boundary] = serializeWorkspace("p7aworkspace");
    }
  }

  $("#p7a-reset").onclick = () => {
    if (p7a.boundary >= 1) {
      p7a.ws[p7a.boundary] = null;
      render();
    }
  };

  $("#p7a-add").onclick = () => {
    const ws = document.getElementById("p7aworkspace");
    if (ws) ws.appendChild(makeAnswerBox({}));
    updateResetVisibility(p7a.boundary);
  };

  $("#p7a-check").onclick = () => {
    resetHint();
    if (!editableSteps.has(p7a.boundary)) return;
    const ws = document.getElementById("p7aworkspace");
    if (!ws) return;
    const boxes = [...ws.querySelectorAll(".vbox")].map((v) => readBoxState(v));
    const ok = isStepCorrect(boxes);
    $("#p7a-status").textContent = ok ? "correct" : "incorrect";
    $("#p7a-status").className = ok ? "ok" : "err";
    MB.flashStatus($("#p7a-status"));
    if (ok) {
      p7a.passes[p7a.boundary] = true;
      p7a.ws[p7a.boundary] = boxes;
      ws.querySelectorAll(".vbox").forEach((v) => MB.disableBoxEditing(v));
      MB.removeBoxDeleteButtons(ws);
      $("#p7a-check").classList.add("hidden");
      $("#p7a-reset").classList.add("hidden");
      $("#p7a-add").classList.add("hidden");
      hint.hide();
      $("#p7a-hint-btn")?.classList.add("hidden");
      MB.pulseNextButton("p7a");
      renderCode();
      pager.update();
    }
  };

  function isStepCorrect(boxes) {
    const expected = canonical(p7a.boundary);
    const by = Object.fromEntries(boxes.map((b) => [b.name, b]));
    const wolf = by.wolf;
    const bear = by.bear;
    const fox = by.fox;
    if (boxes.length !== expected.length) return false;
    if (!by.deer || !by.hare || !wolf) return false;
    if (
      by.deer.type !== "int" ||
      by.hare.type !== "int" ||
      wolf.type !== "int*"
    )
      return false;
    if (!isEmptyVal(by.deer.value || "") || !isEmptyVal(by.hare.value || ""))
      return false;
    if ((wolf.value || "").trim() !== String(p7a.bAddr)) return false;
    if (p7a.boundary >= 6) {
      if (!bear || bear.type !== "int**") return false;
      const pval = (bear.value || "").trim();
      if (pval !== String(p7a.ptrAddr)) return false;
    }
    if (p7a.boundary >= 7) {
      if (!fox || fox.type !== "int*") return false;
      const sval = (fox.value || "").trim();
      if (sval !== String(p7a.bAddr)) return false;
    }
    return true;
  }

  const pager = createStepper({
    prefix: "p7a",
    lines: p7a.lines,
    nextPage: NEXT_PAGE,
    getBoundary: () => p7a.boundary,
    setBoundary: (val) => {
      p7a.boundary = val;
    },
    onBeforeChange: save,
    onAfterChange: () => {
      render();
      updateStatus();
    },
    isStepLocked: (boundary) => {
      if (editableSteps.has(boundary)) return !p7a.passes[boundary];
      if (boundary === p7a.lines.length) return !p7a.passes[p7a.lines.length];
      return false;
    },
    getStepBadge: (step) => {
      if (!editableSteps.has(step)) return "";
      return p7a.passes[step] ? "check" : "note";
    },
  });

  render();
  updateStatus();
  pager.update();
})(window.MB);
