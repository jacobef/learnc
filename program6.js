(function (MB) {
  const {
    $,
    cloneBoxes,
    firstNonEmptyClone,
    renderCodePane,
    restoreWorkspace,
    serializeWorkspace,
    readBoxState,
    isEmptyVal,
    makeAnswerBox,
    createHintController,
    createStepper,
  } = MB;

  const instructions = $("#p5-instructions");
  const solvedMsg = $("#p5-complete");
  const NEXT_PAGE = "program7a.html";

  const p5 = {
    lines: [
      "int hammer;",
      "int drill = 1;",
      "hammer = drill;",
      "drill = 2;",
      "drill = hammer;",
    ],
    boundary: 0,
    xAddr: MB.randAddr("int"),
    yAddr: MB.randAddr("int"),
    ws: Array(6).fill(null),
    snaps: Array(6).fill(null),
    passes: Array(6).fill(false),
    baseline: Array(6).fill(null),
  };

  const hint = createHintController({
    button: "#p5-hint-btn",
    panel: "#p5-hint",
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
    if (!p5.baseline[boundary]) p5.baseline[boundary] = cloneBoxes(state);
    return p5.baseline[boundary];
  }

  function updateResetVisibility(baseline) {
    const resetBtn = $("#p5-reset");
    if (!resetBtn) return;
    const current = serializeWorkspace("p5workspace") || [];
    const changed = Array.isArray(baseline) && !statesEqual(current, baseline);
    resetBtn.classList.toggle("hidden", !changed);
  }

  function attachResetWatcher(wrap, baseline) {
    if (!wrap) return;
    const refresh = () => updateResetVisibility(baseline);
    wrap.addEventListener("input", refresh);
    wrap.addEventListener("click", () => setTimeout(refresh, 0));
    refresh();
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

  function updateInstructions() {
    if (p5.boundary === 0) {
      setInstructions("No instructions for this one. Good luck!");
    } else {
      setInstructions("");
    }
  }

  function buildHint() {
    const ws = document.getElementById("p5workspace");
    const boxes = [...ws.querySelectorAll(".vbox")].map((v) => readBoxState(v));
    const by = Object.fromEntries(boxes.map((b) => [b.name, b]));
    if (p5.boundary === 2) {
      if (!by.hammer)
        return {
          html: 'You still need <code class="tok-name">hammer</code> in the program state.',
        };
      if (!by.drill)
        return {
          html: 'You still need <code class="tok-name">drill</code> in the program state.',
        };
      if (!isEmptyVal(by.hammer.value || ""))
        return {
          html: '<code class="tok-name">hammer</code> should still be empty here.',
        };
      if (isEmptyVal(by.drill.value || ""))
        return {
          html: 'Set <code class="tok-name">drill</code>\'s value to <code class="tok-value">1</code>.',
        };
      if (by.drill.value !== "1")
        return {
          html: 'Line 2 stores <code class="tok-value">1</code> in <code class="tok-name">drill</code>.',
        };
      const ok =
        boxes.length === 2 &&
        by.hammer &&
        by.drill &&
        by.hammer.type === "int" &&
        by.drill.type === "int" &&
        isEmptyVal(by.hammer.value || "") &&
        by.drill.value === "1";
      if (ok)
        return {
          html: 'Looks good. Press <span class="btn-ref">Check</span>.',
        };
    }
    if (p5.boundary === 5) {
      if (!by.hammer || !by.drill)
        return {
          html: 'You still need <code class="tok-name">hammer</code> and <code class="tok-name">drill</code> in the program state.',
        };
      if ((by.drill?.value || "").trim().toLowerCase() === "hammer")
        return {
          html: '<code class="tok-name">drill</code>\'s value should be <code class="tok-name">hammer</code>\'s value, not the literal word \"hammer\".',
        };
      if (isEmptyVal(by.hammer.value || ""))
        return {
          html: '<code class="tok-name">hammer</code> already equals <code class="tok-value">1</code> from the earlier assignments.',
        };
      if (by.hammer.value !== "1")
        return {
          html: '<code class="tok-line">drill = hammer;</code> should modify <code class="tok-name">drill</code>, not <code class="tok-name">hammer</code>.',
        };
      if (isEmptyVal(by.drill.value || ""))
        return {
          html: '<code class="tok-line">hammer = drill;</code> puts <code class="tok-name">drill</code>\'s value into <code class="tok-name">hammer</code>. What should <code class="tok-line">drill = hammer;</code> do?',
        };
      if (by.drill.value === "2" && by.hammer.value === "2")
        return {
          html: 'Remember: this line does not change <code class="tok-name">hammer</code>. Only <code class="tok-name">drill</code> should change here.',
        };
      if (by.drill.value !== "1")
        return {
          html: '<code class="tok-line">hammer = drill;</code> puts <code class="tok-name">drill</code>\'s value into <code class="tok-name">hammer</code>. What should <code class="tok-line">drill = hammer;</code> do?',
        };
      const ok =
        boxes.length === 2 &&
        by.hammer &&
        by.drill &&
        by.hammer.type === "int" &&
        by.drill.type === "int" &&
        by.hammer.value === "1" &&
        by.drill.value === "1";
      if (ok)
        return {
          html: 'Looks good. Press <span class="btn-ref">Check</span>.',
        };
    }
    const hasReset = !!document.getElementById("p5-reset");
    return hasReset
      ? {
          html: 'Your program has a problem that isn\'t covered by a hint. Try starting over on this step by clicking <span class="btn-ref">Reset</span>.',
        }
      : "Your program has a problem that isn't covered by a hint. Sorry.";
  }

  function renderCodePane5() {
    const progress =
      (p5.boundary === 2 && !p5.passes[2]) ||
      (p5.boundary === 5 && !p5.passes[5]);
    renderCodePane($("#p5-code"), p5.lines, p5.boundary, { progress });
  }

  function canonical(boundary) {
    const xAddr = String(p5.xAddr);
    const yAddr = String(p5.yAddr);
    const states = {
      1: [{ name: "hammer", type: "int", value: "empty", address: xAddr }],
      2: [
        { name: "hammer", type: "int", value: "empty", address: xAddr },
        { name: "drill", type: "int", value: "1", address: yAddr },
      ],
      3: [
        { name: "hammer", type: "int", value: "1", address: xAddr },
        { name: "drill", type: "int", value: "1", address: yAddr },
      ],
      4: [
        { name: "hammer", type: "int", value: "1", address: xAddr },
        { name: "drill", type: "int", value: "2", address: yAddr },
      ],
      5: [
        { name: "hammer", type: "int", value: "1", address: xAddr },
        { name: "drill", type: "int", value: "1", address: yAddr },
      ],
    };
    return cloneBoxes(states[boundary] || []);
  }

  function stateFor(boundary) {
    if (boundary <= 0) return [];
    const stored = firstNonEmptyClone(p5.ws[boundary], p5.snaps[boundary]);
    if (stored.length) return cloneBoxes(stored);
    if (boundary === 2 && !p5.passes[2]) {
      const prev = firstNonEmptyClone(p5.ws[1], p5.snaps[1]);
      if (prev.length) return cloneBoxes(prev);
    }
    if (boundary === 5) {
      const prev = firstNonEmptyClone(p5.ws[4], p5.snaps[4]);
      if (prev.length) return cloneBoxes(prev);
    }
    return canonical(boundary);
  }

  function render() {
    renderCodePane5();
    updateInstructions();
    const stage = $("#p5-stage");
    stage.innerHTML = "";
    const boundary = p5.boundary;
    const atSolved =
      (boundary === 2 && p5.passes[2]) || (boundary === 5 && p5.passes[5]);
    if (atSolved) {
      $("#p5-status").textContent = "correct";
      $("#p5-status").className = "ok";
    } else {
      $("#p5-status").textContent = "";
      $("#p5-status").className = "muted";
    }
    $("#p5-check").classList.add("hidden");
    $("#p5-reset").classList.add("hidden");
    $("#p5-add").classList.add("hidden");

    resetHint();
    hint.setButtonHidden(
      !((boundary === 2 && !p5.passes[2]) || (boundary === 5 && !p5.passes[5])),
    );
    if (boundary > 0) {
      const editable =
        (boundary === 2 && !p5.passes[2]) || (boundary === 5 && !p5.passes[5]);
      const state = stateFor(boundary);
      const wrap = restoreWorkspace(p5.ws[boundary], state, "p5workspace", {
        editable,
        deletable: editable && boundary === 2,
      });
      stage.appendChild(wrap);
      if (editable) {
        $("#p5-check").classList.remove("hidden");
        if (boundary === 2) $("#p5-add").classList.remove("hidden");
        const baseline = ensureBaseline(boundary, state);
        attachResetWatcher(wrap, baseline);
      } else if (state.length) {
        const snapshot = cloneBoxes(state);
        p5.ws[boundary] = snapshot;
        p5.snaps[boundary] = snapshot;
      }
    }
  }

  function save() {
    if (p5.boundary >= 1 && p5.boundary <= p5.lines.length) {
      p5.ws[p5.boundary] = serializeWorkspace("p5workspace");
    }
  }

  $("#p5-reset").onclick = () => {
    if (p5.boundary === 2) {
      p5.ws[2] = null;
      p5.snaps[2] = null;
      p5.passes[2] = false;
      render();
      return;
    }
    if (p5.boundary === 5) {
      p5.ws[5] = null;
      p5.snaps[5] = null;
      p5.passes[5] = false;
      render();
    }
  };

  $("#p5-add").onclick = () => {
    const ws = document.getElementById("p5workspace");
    if (!ws) return;
    ws.appendChild(makeAnswerBox({}));
    updateResetVisibility(p5.baseline[p5.boundary]);
  };

  $("#p5-check").onclick = () => {
    resetHint();
    if (p5.boundary !== 2 && p5.boundary !== 5) return;
    const ws = document.getElementById("p5workspace");
    if (!ws) return;
    const boxes = [...ws.querySelectorAll(".vbox")].map((v) => readBoxState(v));
    const by = Object.fromEntries(boxes.map((b) => [b.name, b]));
    const hammer = by.hammer;
    const drill = by.drill;
    const allTypesOk = boxes.every((b) => b.type === "int");
    let ok = false;
    if (p5.boundary === 2) {
      ok =
        boxes.length === 2 &&
        hammer &&
        drill &&
        allTypesOk &&
        isEmptyVal(hammer.value || "") &&
        drill.value === "1";
    } else {
      ok =
        boxes.length === 2 &&
        hammer &&
        drill &&
        allTypesOk &&
        hammer.value === "1" &&
        drill.value === "1";
    }

    $("#p5-status").textContent = ok ? "correct" : "incorrect";
    $("#p5-status").className = ok ? "ok" : "err";
    MB.flashStatus($("#p5-status"));
    if (ok) {
      const snap = serializeWorkspace("p5workspace");
      if (Array.isArray(snap)) {
        p5.ws[p5.boundary] = snap;
        p5.snaps[p5.boundary] = snap;
      } else {
        p5.ws[p5.boundary] = boxes;
        p5.snaps[p5.boundary] = boxes;
      }
      ws.querySelectorAll(".vbox").forEach((v) => MB.disableBoxEditing(v));
      MB.removeBoxDeleteButtons(ws);
      p5.passes[p5.boundary] = true;
      renderCodePane5();
      $("#p5-check").classList.add("hidden");
      $("#p5-reset").classList.add("hidden");
      hint.hide();
      $("#p5-hint-btn")?.classList.add("hidden");
      MB.pulseNextButton("p5");
      pager.update();
    }
  };
  const pager = createStepper({
    prefix: "p5",
    lines: p5.lines,
    nextPage: NEXT_PAGE,
    getBoundary: () => p5.boundary,
    setBoundary: (val) => {
      p5.boundary = val;
    },
    onBeforeChange: save,
    onAfterChange: render,
    isStepLocked: (boundary) => {
      if (boundary === 2) return !p5.passes[2];
      if (boundary === 5) return !p5.passes[5];
      return false;
    },
    getStepBadge: (step) => {
      if (step === 2) return p5.passes[2] ? "check" : "note";
      if (step === 5) return p5.passes[5] ? "check" : "note";
      return "";
    },
  });

  render();
  pager.update();
})(window.MB);
