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

  const instructions = $("#p12-instructions");
  const NEXT_PAGE = "sandbox.html?finished=1";

  const p12 = {
    lines: [
      "int spark = 6;",
      "int ember; // will be set later",
      "int cinder = 2 + 3 /* 5 */ * (4 - 1);",
      "int* flame = &spark;",
      "ember = cinder / 3 + - 1;",
      "*flame = spark * 4;",
      "int* smolder = &ember;",
      "int** blaze = &smolder;",
      "int*** inferno = &blaze;",
      "**inferno = &cinder;",
      "int ash = **blaze-*flame / ember + (***inferno == 24);",
    ],
    boundary: 0,
    addrs: {},
    ws: {},
    passes: {
      3: false,
      5: false,
      6: false,
      10: false,
      11: false,
    },
    baseline: {},
    instructionsEnabled: true,
  };

  const statementRanges = [];
  const editableSteps = new Set([3, 5, 6, 10, 11]);

  const hint = createHintController({
    button: "#p12-hint-btn",
    panel: "#p12-hint",
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
    if (!p12.addrs[name]) p12.addrs[name] = randAddr(type);
    return p12.addrs[name];
  }

  function captureAddrs(boxes) {
    if (!Array.isArray(boxes)) return;
    boxes.forEach((box) => {
      const name = (box?.name || "").trim();
      if (!name) return;
      const addrValue = box.addr ?? box.address ?? null;
      if (addrValue != null) p12.addrs[name] = String(addrValue);
    });
  }

  function p12Save() {
    if (!editableSteps.has(p12.boundary)) return;
    const snap = serializeWorkspace("p12workspace");
    if (Array.isArray(snap) && snap.length) {
      p12.ws[p12.boundary] = snap;
      captureAddrs(snap);
    } else {
      p12.ws[p12.boundary] = null;
    }
  }

  function valueForSpark(boundary) {
    if (boundary >= 6) return "24";
    return "6";
  }

  function valueForEmber(boundary) {
    if (boundary >= 5) return "2";
    return "empty";
  }

  function valueForFlame() {
    return String(addr("spark", "int"));
  }

  function valueForSmolder(boundary) {
    if (boundary >= 10) return String(addr("cinder", "int"));
    return String(addr("ember", "int"));
  }

  function canonical(boundary) {
    const boxes = [];
    const pushBox = (spec) => {
      boxes.push({
        ...spec,
        allowNameEdit: false,
        allowTypeEdit: false,
      });
    };
    if (boundary >= 1) {
      pushBox({
        address: String(addr("spark", "int")),
        type: "int",
        value: valueForSpark(boundary),
        name: "spark",
      });
    }
    if (boundary >= 2) {
      pushBox({
        address: String(addr("ember", "int")),
        type: "int",
        value: valueForEmber(boundary),
        name: "ember",
      });
    }
    if (boundary >= 3) {
      pushBox({
        address: String(addr("cinder", "int")),
        type: "int",
        value: "11",
        name: "cinder",
      });
    }
    if (boundary >= 4) {
      pushBox({
        address: String(addr("flame", "int*")),
        type: "int*",
        value: valueForFlame(),
        name: "flame",
      });
    }
    if (boundary >= 7) {
      pushBox({
        address: String(addr("smolder", "int*")),
        type: "int*",
        value: valueForSmolder(boundary),
        name: "smolder",
      });
    }
    if (boundary >= 8) {
      pushBox({
        address: String(addr("blaze", "int**")),
        type: "int**",
        value: String(addr("smolder", "int*")),
        name: "blaze",
      });
    }
    if (boundary >= 9) {
      pushBox({
        address: String(addr("inferno", "int***")),
        type: "int***",
        value: String(addr("blaze", "int**")),
        name: "inferno",
      });
    }
    if (boundary >= 11) {
      pushBox({
        address: String(addr("ash", "int")),
        type: "int",
        value: "-1",
        name: "ash",
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
    if (p12.boundary === 0) {
      setInstructions("No instructions for this one. Good luck!");
      return;
    }
    if (p12.boundary === 11) {
      setInstructions("Sorry.");
      return;
    }
    if (p12.boundary === p12.lines.length && p12.passes[p12.lines.length]) {
      setInstructions("Program solved!");
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
    if (!p12.baseline[boundary]) p12.baseline[boundary] = cloneBoxes(state);
    return p12.baseline[boundary];
  }

  function updateResetVisibility(boundary) {
    const resetBtn = $("#p12-reset");
    if (!resetBtn) return;
    const baseline = p12.baseline[boundary];
    const current = serializeWorkspace("p12workspace") || [];
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
    if (boundary === 3) {
      return [
        { name: "spark", type: "int", value: "6" },
        { name: "ember", type: "int", value: "empty" },
        { name: "cinder", type: "int", value: "11" },
      ];
    }
    if (boundary === 5) {
      return [
        { name: "spark", type: "int", value: "6" },
        { name: "ember", type: "int", value: "2" },
        { name: "cinder", type: "int", value: "11" },
        { name: "flame", type: "int*", value: valueForFlame() },
      ];
    }
    if (boundary === 6) {
      return [
        { name: "spark", type: "int", value: "24" },
        { name: "ember", type: "int", value: "2" },
        { name: "cinder", type: "int", value: "11" },
        { name: "flame", type: "int*", value: valueForFlame() },
      ];
    }
    if (boundary === 9) {
      return [
        { name: "spark", type: "int", value: "24" },
        { name: "ember", type: "int", value: "2" },
        { name: "cinder", type: "int", value: "11" },
        { name: "flame", type: "int*", value: valueForFlame() },
        { name: "smolder", type: "int*", value: valueForSmolder(boundary) },
        { name: "blaze", type: "int**", value: String(addr("smolder", "int*")) },
        { name: "inferno", type: "int***", value: String(addr("blaze", "int**")) },
      ];
    }
    if (boundary === 10) {
      return [
        { name: "spark", type: "int", value: "24" },
        { name: "ember", type: "int", value: "2" },
        { name: "cinder", type: "int", value: "11" },
        { name: "flame", type: "int*", value: valueForFlame() },
        { name: "smolder", type: "int*", value: valueForSmolder(boundary) },
        { name: "blaze", type: "int**", value: String(addr("smolder", "int*")) },
        { name: "inferno", type: "int***", value: String(addr("blaze", "int**")) },
      ];
    }
    if (boundary === 11) {
      return [
        { name: "spark", type: "int", value: "24" },
        { name: "ember", type: "int", value: "2" },
        { name: "cinder", type: "int", value: "11" },
        { name: "flame", type: "int*", value: valueForFlame() },
        { name: "smolder", type: "int*", value: valueForSmolder(boundary) },
        { name: "blaze", type: "int**", value: String(addr("smolder", "int*")) },
        { name: "inferno", type: "int***", value: String(addr("blaze", "int**")) },
        { name: "ash", type: "int", value: "-1" },
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
    const byName = Object.fromEntries(
      boxes.map((b) => [(b.name || "").trim(), b]),
    );
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
              html: `<code class="tok-name">${need.name}</code> should be <code class="tok-value">empty</code>.`,
            },
          };
        }
        continue;
      }
      if (isEmptyVal(box.value || "")) {
        return {
          ok: false,
          message: {
            html: `${need.name}'s value shouldn't be blank.`,
          },
        };
      }
      let expectedValue = need.value;
      if (boundary === 5 && need.name === "flame") {
        const sparkAddr = byName.spark?.addr ?? byName.spark?.address;
        if (sparkAddr != null) expectedValue = String(sparkAddr);
      }
      if (box.value !== expectedValue) {
        return {
          ok: false,
          message: {
            html: `<code class="tok-name">${need.name}</code> should be <code class="tok-value">${expectedValue}</code>.`,
          },
        };
      }
    }
    return { ok: true };
  }

  function buildHint() {
    if (!editableSteps.has(p12.boundary)) {
      return {
        html: 'Use <span class="btn-ref">Run line 1 ▶</span> to reach the editable lines.',
      };
    }
    const ws = document.getElementById("p12workspace");
    if (!ws)
      return {
        html: 'Use <span class="btn-ref">Run line 1 ▶</span> to reach the editable lines.',
      };
    const boxes = [...ws.querySelectorAll(".vbox")].map((v) =>
      readBoxState(v),
    );
    const by = Object.fromEntries(boxes.map((b) => [b.name, b]));
    if (p12.boundary === 3) {
      if (!by.spark || !by.ember || !by.cinder)
        return {
          html: "Keep <code class=\"tok-name\">spark</code> and <code class=\"tok-name\">ember</code>, and add <code class=\"tok-name\">cinder</code>.",
        };
      if (by.cinder.value !== "11")
        return {
          html: "Compute <code class=\"tok-name\">cinder</code> using multiplication before addition.",
        };
    }
    if (p12.boundary === 5) {
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
          html: "Line 5 is <code class=\"tok-line\">ember = cinder / 3 + - 1</code>.",
        };
      const sparkAddr = by.spark?.addr ?? by.spark?.address ?? String(addr("spark", "int"));
      if (by.flame.value !== String(sparkAddr))
        return {
          html: "<code class=\"tok-name\">flame</code> should store <code class=\"tok-name\">spark</code>'s address.",
        };
    }
    if (p12.boundary === 6) {
      if (!by.spark || !by.ember || !by.flame)
        return {
          html: "Make sure <code class=\"tok-name\">spark</code>, <code class=\"tok-name\">ember</code>, and <code class=\"tok-name\">flame</code> are present.",
        };
      if (by.spark.value !== "24")
        return {
          html: "<code class=\"tok-line\">*flame = spark * 4</code> updates <code class=\"tok-name\">spark</code>.",
        };
    }
    if (p12.boundary === 10) {
      if (!by.smolder)
        return {
          html: "Add <code class=\"tok-name\">smolder</code> as an <code class=\"tok-type\">int*</code> pointing at <code class=\"tok-name\">ember</code>.",
        };
      if (!by.blaze)
        return {
          html: "Keep <code class=\"tok-name\">blaze</code> as an <code class=\"tok-type\">int**</code> pointing at <code class=\"tok-name\">smolder</code>.",
        };
      if (!by.inferno)
        return {
          html: "Add <code class=\"tok-name\">inferno</code> as an <code class=\"tok-type\">int***</code> pointing at <code class=\"tok-name\">blaze</code>.",
        };
      if (!by.flame)
        return {
          html: "Keep <code class=\"tok-name\">flame</code> in the program state.",
        };
      if (by.smolder.value !== String(addr("cinder", "int")))
        return {
          html: "<code class=\"tok-line\">**inferno = &cinder</code> updates <code class=\"tok-name\">smolder</code> to hold <code class=\"tok-name\">cinder</code>'s address.",
        };
    }
    if (p12.boundary === 11) {
      if (!by.ash)
        return {
          html: "Add <code class=\"tok-name\">ash</code> from <code class=\"tok-line\">int ash = **blaze-*flame / ember + (***inferno == 24)</code>.",
        };
      if (by.ash.value !== "-1")
        return {
          html: "Mind precedence and spacing, then update <code class=\"tok-name\">ash</code>.",
        };
    }
    const verdict = validateWorkspace(p12.boundary, boxes);
    if (verdict.ok)
      return { html: 'Looks good. Press <span class="btn-ref">Check</span>.' };
    return {
      html: "Something is off. Double-check variable names, types, and values.",
    };
  }

  function renderCodePane10() {
    const progress = editableSteps.has(p12.boundary) && !p12.passes[p12.boundary];
    let progressIndex;
    let progressRange;
    let doneBoundary;
    if (progress) {
      const range = rangeEndingAt(p12.boundary);
      if (range) {
        doneBoundary = range.start;
        progressIndex = range.start;
        progressRange = [range.start, range.end - 1];
      }
    }
    renderCodePane($("#p12-code"), p12.lines, p12.boundary, {
      progress,
      progressIndex,
      progressRange,
      doneBoundary,
    });
  }

  function p12Render() {
    renderCodePane10();
    updateInstructions();
    const stage = $("#p12-stage");
    stage.innerHTML = "";
    const addBtn = $("#p12-add");
    const checkBtn = $("#p12-check");
    const resetBtn = $("#p12-reset");
    const status = $("#p12-status");
    addBtn?.classList.add("hidden");
    checkBtn?.classList.add("hidden");
    resetBtn?.classList.add("hidden");
    hint.setButtonHidden(true);
    resetHint();

    if (editableSteps.has(p12.boundary) && p12.passes[p12.boundary]) {
      if (status) {
        status.textContent = "correct";
        status.className = "ok";
      }
    } else if (status) {
      status.textContent = "";
      status.className = "muted";
    }

    if (p12.boundary <= 0) return;
    const editable = editableSteps.has(p12.boundary) && !p12.passes[p12.boundary];
    const defaults = defaultsFor(p12.boundary);
    const wrap = restoreWorkspace(p12.ws[p12.boundary], defaults, "p12workspace", {
      editable,
      deletable: editable,
      allowNameEdit: null,
      allowTypeEdit: editable ? null : false,
    });
    stage.appendChild(wrap);
    if (editable) {
      addBtn?.classList.remove("hidden");
      checkBtn?.classList.remove("hidden");
      hint.setButtonHidden(false);
      ensureBaseline(p12.boundary, defaults);
      attachResetWatcher(wrap, p12.boundary);
    }
  }

  $("#p12-reset").onclick = () => {
    if (!editableSteps.has(p12.boundary)) return;
    p12.ws[p12.boundary] = null;
    p12.passes[p12.boundary] = false;
    p12.baseline[p12.boundary] = null;
    p12Render();
    pager.update();
  };

  $("#p12-check").onclick = () => {
    resetHint();
    if (!editableSteps.has(p12.boundary)) return;
    const ws = document.getElementById("p12workspace");
    if (!ws) return;
    const boxes = [...ws.querySelectorAll(".vbox")].map((v) =>
      readBoxState(v),
    );
    const verdict = validateWorkspace(p12.boundary, boxes);
    const status = $("#p12-status");
    if (status) {
      status.textContent = verdict.ok ? "correct" : "incorrect";
      status.className = verdict.ok ? "ok" : "err";
      flashStatus(status);
    }
    if (!verdict.ok) return;
    p12.passes[p12.boundary] = true;
    const snap = serializeWorkspace("p12workspace");
    if (Array.isArray(snap)) {
      p12.ws[p12.boundary] = snap;
      captureAddrs(snap);
    }
    ws.querySelectorAll(".vbox").forEach((v) => disableBoxEditing(v));
    removeBoxDeleteButtons(ws);
    $("#p12-check").classList.add("hidden");
    $("#p12-reset").classList.add("hidden");
    $("#p12-add").classList.add("hidden");
    hint.hide();
    $("#p12-hint-btn")?.classList.add("hidden");
    pulseNextButton("p12");
    p12Render();
    pager.update();
  };

  const pager = createStepper({
    prefix: "p12",
    lines: p12.lines,
    nextPage: NEXT_PAGE,
    endLabel: "Finish",
    getBoundary: () => p12.boundary,
    setBoundary: (val) => {
      p12.boundary = val;
    },
    onBeforeChange: p12Save,
    onAfterChange: p12Render,
    isStepLocked: (boundary) =>
      editableSteps.has(boundary) && !p12.passes[boundary],
    getStepBadge: (step) => {
      if (!editableSteps.has(step)) return "";
      return p12.passes[step] ? "check" : "note";
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

  $("#p12-add").onclick = () => {
    const ws = document.getElementById("p12workspace");
    if (!ws) return;
    ws.appendChild(makeAnswerBox({}));
    updateResetVisibility(p12.boundary);
  };

  instructions?.addEventListener("click", (event) => {
    const target = event.target;
    if (!(target instanceof Element)) return;
    if (!target.matches("[data-action='disable-instructions']")) return;
    event.preventDefault();
    p12.instructionsEnabled = false;
    updateInstructions();
  });

  p12Render();
  pager.update();
})(window.MB);
