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
  const NEXT_PAGE = "sandbox.html";
  const FINISH_PARAM = "finished";

  const p11 = {
    lines: [
      "int spark = 6;",
      "int ember; // will be set later",
      "int cinder = 2 + 3 /* tricky */ * (4 - 1);",
      "int* flame = &spark;",
      "ember = cinder / 3 + -1;",
      "*flame = ember * 4;",
      "int** blaze = &flame;",
      "int ash = **blaze + 5;",
      "int soot = ash == 13;",
      "spark = (spark + ember) / 2 /* average */ ;",
      "/* smoke comes next */",
      "int smoke = ash + /* tiny */ soot;",
    ],
    boundary: 0,
    addrs: {},
    ws: {},
    passes: {
      5: false,
      6: false,
      8: false,
      9: false,
      10: false,
      12: false,
    },
    baseline: {},
    instructionsEnabled: true,
  };

  const statementRanges = [];
  const editableSteps = new Set([5, 6, 8, 9, 10, 12]);

  const hint = createHintController({
    button: "#p11-hint-btn",
    panel: "#p11-hint",
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

  function addr(name, type = "int") {
    if (!p11.addrs[name]) p11.addrs[name] = randAddr(type);
    return p11.addrs[name];
  }

  function captureAddrs(boxes) {
    if (!Array.isArray(boxes)) return;
    boxes.forEach((box) => {
      const name = (box?.name || "").trim();
      if (!name) return;
      const addrValue = box.addr ?? box.address ?? null;
      if (addrValue != null) p11.addrs[name] = String(addrValue);
    });
  }

  function p11Save() {
    if (!editableSteps.has(p11.boundary)) return;
    const snap = serializeWorkspace("p11workspace");
    if (Array.isArray(snap) && snap.length) {
      p11.ws[p11.boundary] = snap;
      captureAddrs(snap);
    } else {
      p11.ws[p11.boundary] = null;
    }
  }

  function valueForSpark(boundary) {
    if (boundary >= 10) return "5";
    if (boundary >= 6) return "8";
    return "6";
  }

  function valueForEmber(boundary) {
    if (boundary >= 5) return "2";
    return "empty";
  }

  function canonical(boundary) {
    const boxes = [];
    if (boundary >= 1) {
      boxes.push({
        address: String(addr("spark", "int")),
        type: "int",
        value: valueForSpark(boundary),
        name: "spark",
      });
    }
    if (boundary >= 2) {
      boxes.push({
        address: String(addr("ember", "int")),
        type: "int",
        value: valueForEmber(boundary),
        name: "ember",
      });
    }
    if (boundary >= 3) {
      boxes.push({
        address: String(addr("cinder", "int")),
        type: "int",
        value: "11",
        name: "cinder",
      });
    }
    if (boundary >= 4) {
      boxes.push({
        address: String(addr("flame", "int*")),
        type: "int*",
        value: String(addr("spark", "int")),
        name: "flame",
      });
    }
    if (boundary >= 7) {
      boxes.push({
        address: String(addr("blaze", "int**")),
        type: "int**",
        value: String(addr("flame", "int*")),
        name: "blaze",
      });
    }
    if (boundary >= 8) {
      boxes.push({
        address: String(addr("ash", "int")),
        type: "int",
        value: "13",
        name: "ash",
      });
    }
    if (boundary >= 9) {
      boxes.push({
        address: String(addr("soot", "int")),
        type: "int",
        value: "1",
        name: "soot",
      });
    }
    if (boundary >= 12) {
      boxes.push({
        address: String(addr("smoke", "int")),
        type: "int",
        value: "14",
        name: "smoke",
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
    if (p11.boundary === p11.lines.length && p11.passes[p11.lines.length]) {
      setInstructions("Program solved!");
      return;
    }
    if (!p11.instructionsEnabled) {
      setInstructions("");
      return;
    }

    const runLabel = runLabelForBoundary(p11.boundary);
    if (p11.boundary === 0) {
      setInstructions(
        prependTopStepperNotice(
          "p11",
          'This review uses every concept so far. <button type="button" class="ub-explain-link" data-action="disable-instructions">Click here</button> to hide instructions, or click <span class="btn-ref">Run line 1 ▶</span> to begin.',
          { html: true },
        ),
        { html: true },
      );
      return;
    }
    if (p11.boundary >= 1 && p11.boundary <= 4) {
      setInstructions(
        "Declarations create boxes; initializations create boxes and fill them immediately.",
      );
      return;
    }
    if (p11.boundary >= 5 && p11.boundary <= 6) {
      setInstructions(
        "Remember operator precedence and that integer division rounds toward 0.",
      );
      return;
    }
    if (p11.boundary >= 7 && p11.boundary <= 8) {
      setInstructions(
        "Pointers store addresses; dereferencing follows those addresses.",
      );
      return;
    }
    if (p11.boundary === 9) {
      setInstructions("Equality comparisons evaluate to 1 or 0.");
      return;
    }
    if (p11.boundary >= 10) {
      setInstructions("Comments can appear inside statements, but they don't affect order of operations.");
      return;
    }
    setInstructions(
      `Click <span class="btn-ref">${runLabel}</span> to continue.`,
      { html: true },
    );
  }

  function normalizeState(list) {
    if (!Array.isArray(list)) return [];
    return list
      .map((b) => ({
        name: (b.name || "").trim(),
        type: (b.type || "").trim(),
        value: String(b.value ?? "").trim(),
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

  function expectedFor(boundary) {
    if (boundary === 5) {
      return [
        { name: "spark", type: "int", value: "6" },
        { name: "ember", type: "int", value: "2" },
        { name: "cinder", type: "int", value: "11" },
        { name: "flame", type: "int*", value: String(addr("spark", "int")) },
      ];
    }
    if (boundary === 6) {
      return [
        { name: "spark", type: "int", value: "8" },
        { name: "ember", type: "int", value: "2" },
        { name: "cinder", type: "int", value: "11" },
        { name: "flame", type: "int*", value: String(addr("spark", "int")) },
      ];
    }
    if (boundary === 8) {
      return [
        { name: "spark", type: "int", value: "8" },
        { name: "ember", type: "int", value: "2" },
        { name: "cinder", type: "int", value: "11" },
        { name: "flame", type: "int*", value: String(addr("spark", "int")) },
        { name: "blaze", type: "int**", value: String(addr("flame", "int*")) },
        { name: "ash", type: "int", value: "13" },
      ];
    }
    if (boundary === 9) {
      return [
        { name: "spark", type: "int", value: "8" },
        { name: "ember", type: "int", value: "2" },
        { name: "cinder", type: "int", value: "11" },
        { name: "flame", type: "int*", value: String(addr("spark", "int")) },
        { name: "blaze", type: "int**", value: String(addr("flame", "int*")) },
        { name: "ash", type: "int", value: "13" },
        { name: "soot", type: "int", value: "1" },
      ];
    }
    if (boundary === 10) {
      return [
        { name: "spark", type: "int", value: "5" },
        { name: "ember", type: "int", value: "2" },
        { name: "cinder", type: "int", value: "11" },
        { name: "flame", type: "int*", value: String(addr("spark", "int")) },
        { name: "blaze", type: "int**", value: String(addr("flame", "int*")) },
        { name: "ash", type: "int", value: "13" },
        { name: "soot", type: "int", value: "1" },
      ];
    }
    if (boundary === 12) {
      return [
        { name: "spark", type: "int", value: "5" },
        { name: "ember", type: "int", value: "2" },
        { name: "cinder", type: "int", value: "11" },
        { name: "flame", type: "int*", value: String(addr("spark", "int")) },
        { name: "blaze", type: "int**", value: String(addr("flame", "int*")) },
        { name: "ash", type: "int", value: "13" },
        { name: "soot", type: "int", value: "1" },
        { name: "smoke", type: "int", value: "14" },
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
    const ws = document.getElementById("p11workspace");
    if (!ws)
      return {
        html: 'Use <span class="btn-ref">Run line 1 ▶</span> to reach the editable lines.',
      };
    const boxes = [...ws.querySelectorAll(".vbox")].map((v) =>
      readBoxState(v),
    );
    const by = Object.fromEntries(boxes.map((b) => [b.name, b]));
    if (p11.boundary === 5) {
      if (!by.spark || !by.ember || !by.cinder || !by.flame)
        return {
          html: "You should have <code class=\"tok-name\">spark</code>, <code class=\"tok-name\">ember</code>, <code class=\"tok-name\">cinder</code>, and <code class=\"tok-name\">flame</code> in the program state.",
        };
      if (by.cinder.value !== "11")
        return {
          html: "Compute <code class=\"tok-name\">cinder</code> using multiplication before addition.",
        };
      if (by.ember.value !== "2")
        return {
          html: "Line 5 is <code class=\"tok-line\">ember = cinder / 3 + -1</code>.",
        };
      if (by.flame.value !== String(addr("spark", "int")))
        return {
          html: "<code class=\"tok-name\">flame</code> should store <code class=\"tok-name\">spark</code>'s address.",
        };
    }
    if (p11.boundary === 6) {
      if (!by.spark || !by.ember || !by.flame)
        return {
          html: "Make sure <code class=\"tok-name\">spark</code>, <code class=\"tok-name\">ember</code>, and <code class=\"tok-name\">flame</code> are present.",
        };
      if (by.spark.value !== "8")
        return {
          html: "<code class=\"tok-line\">*flame = ember * 4</code> updates <code class=\"tok-name\">spark</code>.",
        };
    }
    if (p11.boundary === 8) {
      if (!by.blaze)
        return {
          html: "Add <code class=\"tok-name\">blaze</code> as an <code class=\"tok-type\">int**</code> pointing at <code class=\"tok-name\">flame</code>.",
        };
      if (!by.ash)
        return {
          html: "Add <code class=\"tok-name\">ash</code> from <code class=\"tok-line\">int ash = **blaze + 5</code>.",
        };
      if (by.ash.value !== "13")
        return {
          html: "<code class=\"tok-name\">**blaze</code> is the same value as <code class=\"tok-name\">spark</code> here.",
        };
    }
    if (p11.boundary === 9) {
      if (!by.soot)
        return {
          html: "Add <code class=\"tok-name\">soot</code> from <code class=\"tok-line\">ash == 13</code>.",
        };
      if (by.soot.value !== "1")
        return {
          html: "Comparisons evaluate to 1 (true) or 0 (false).",
        };
    }
    if (p11.boundary === 10) {
      if (!by.spark || !by.ember)
        return {
          html: "Keep <code class=\"tok-name\">spark</code> and <code class=\"tok-name\">ember</code> in the program state.",
        };
      if (by.spark.value !== "5")
        return {
          html: "Line 10 averages <code class=\"tok-name\">spark</code> and <code class=\"tok-name\">ember</code> using parentheses.",
        };
    }
    if (p11.boundary === 12) {
      if (!by.smoke)
        return {
          html: "Add <code class=\"tok-name\">smoke</code> from <code class=\"tok-line\">ash + soot</code>.",
        };
      if (by.smoke.value !== "14")
        return {
          html: "<code class=\"tok-name\">ash</code> is 13 and <code class=\"tok-name\">soot</code> is 1.",
        };
    }
    const verdict = validateWorkspace(p11.boundary, boxes);
    if (verdict.ok)
      return { html: 'Looks good. Press <span class="btn-ref">Check</span>.' };
    return {
      html: "Something is off. Double-check variable names, types, and values.",
    };
  }

  function renderCodePane10() {
    const progress = editableSteps.has(p11.boundary) && !p11.passes[p11.boundary];
    let progressIndex;
    let progressRange;
    let doneBoundary;
    if (progress) {
      const range = rangeEndingAt(p11.boundary);
      if (range) {
        doneBoundary = range.start;
        progressIndex = range.start;
        progressRange = [range.start, range.end - 1];
      }
    }
    renderCodePane($("#p11-code"), p11.lines, p11.boundary, {
      progress,
      progressIndex,
      progressRange,
      doneBoundary,
    });
  }

  function p11Render() {
    renderCodePane10();
    updateInstructions();
    const stage = $("#p11-stage");
    stage.innerHTML = "";
    const addBtn = $("#p11-add");
    const checkBtn = $("#p11-check");
    const resetBtn = $("#p11-reset");
    const status = $("#p11-status");
    addBtn?.classList.add("hidden");
    checkBtn?.classList.add("hidden");
    resetBtn?.classList.add("hidden");
    hint.setButtonHidden(true);
    resetHint();

    if (editableSteps.has(p11.boundary) && p11.passes[p11.boundary]) {
      if (status) {
        status.textContent = "correct";
        status.className = "ok";
      }
    } else if (status) {
      status.textContent = "";
      status.className = "muted";
    }

    if (p11.boundary <= 0) return;
    const editable = editableSteps.has(p11.boundary) && !p11.passes[p11.boundary];
    const defaults = defaultsFor(p11.boundary);
    const wrap = restoreWorkspace(p11.ws[p11.boundary], defaults, "p11workspace", {
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
      ensureBaseline(p11.boundary, defaults);
      attachResetWatcher(wrap, p11.boundary);
    }
  }

  $("#p11-reset").onclick = () => {
    if (!editableSteps.has(p11.boundary)) return;
    p11.ws[p11.boundary] = null;
    p11.passes[p11.boundary] = false;
    p11.baseline[p11.boundary] = null;
    p11Render();
    pager.update();
  };

  $("#p11-check").onclick = () => {
    resetHint();
    if (!editableSteps.has(p11.boundary)) return;
    const ws = document.getElementById("p11workspace");
    if (!ws) return;
    const boxes = [...ws.querySelectorAll(".vbox")].map((v) =>
      readBoxState(v),
    );
    const verdict = validateWorkspace(p11.boundary, boxes);
    const status = $("#p11-status");
    if (status) {
      status.textContent = verdict.ok ? "correct" : "incorrect";
      status.className = verdict.ok ? "ok" : "err";
      flashStatus(status);
    }
    if (!verdict.ok) return;
    p11.passes[p11.boundary] = true;
    const snap = serializeWorkspace("p11workspace");
    if (Array.isArray(snap)) {
      p11.ws[p11.boundary] = snap;
      captureAddrs(snap);
    }
    ws.querySelectorAll(".vbox").forEach((v) => disableBoxEditing(v));
    removeBoxDeleteButtons(ws);
    $("#p11-check").classList.add("hidden");
    $("#p11-reset").classList.add("hidden");
    $("#p11-add").classList.add("hidden");
    hint.hide();
    $("#p11-hint-btn")?.classList.add("hidden");
    pulseNextButton("p11");
    p11Render();
    pager.update();
  };

  const pager = createStepper({
    prefix: "p11",
    lines: p11.lines,
    nextPage: NEXT_PAGE,
    endLabel: "Finish",
    getBoundary: () => p11.boundary,
    setBoundary: (val) => {
      p11.boundary = val;
    },
    onBeforeChange: p11Save,
    onAfterChange: p11Render,
    isStepLocked: (boundary) =>
      editableSteps.has(boundary) && !p11.passes[boundary],
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

  $("#p11-add").onclick = () => {
    const ws = document.getElementById("p11workspace");
    if (!ws) return;
    ws.appendChild(makeAnswerBox({}));
    updateResetVisibility(p11.boundary);
  };

  instructions?.addEventListener("click", (event) => {
    const target = event.target;
    if (!(target instanceof Element)) return;
    if (!target.matches("[data-action='disable-instructions']")) return;
    event.preventDefault();
    p11.instructionsEnabled = false;
    updateInstructions();
  });

  $("#p11-next")?.addEventListener("click", (event) => {
    if (pager.boundary() !== p11.lines.length || !p11.passes[p11.lines.length])
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

  p11Render();
  pager.update();
})(window.MB);
