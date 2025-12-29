(function (MB) {
  const {
    $,
    el,
    randAddr,
    renderCodePane,
    vbox,
    readBoxState,
    isEmptyVal,
    makeAnswerBox,
    serializeWorkspace,
    restoreWorkspace,
    cloneStateBoxes,
    ensureBox,
    createHintController,
    createStepper,
  } = MB;

  const solvedMsg = $("#p3-complete");
  const NEXT_PAGE = "program4.html";

  const p3 = {
    lines: [
      "int north;",
      "int south = -5;",
      "north = 5;",
      "int east = 9;",
      "int west = -9;",
    ],
    boundary: 0,
    aAddr: randAddr("int"),
    bAddr: randAddr("int"),
    cAddr: randAddr("int"),
    ws1: null,
    ws3: null,
    ws5: null,
    pass1: false,
    pass3: false,
    pass5: false,
    baseline: {},
  };

  const hint = createHintController({
    button: "#p3-hint-btn",
    panel: "#p3-hint",
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
    if (!p3.baseline[boundary]) p3.baseline[boundary] = cloneStateBoxes(state);
    return p3.baseline[boundary];
  }

  function updateResetVisibility(boundary) {
    const resetBtn = $("#p3-reset");
    if (!resetBtn) return;
    const baseline = p3.baseline[boundary];
    const current = serializeWorkspace("p3workspace") || [];
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
    const ws = document.getElementById("p3workspace");
    if (!ws)
      return {
        html: 'Use <span class="btn-ref">+ New variable</span> to add the variables you need to the program state.',
      };
    const boxes = [...ws.querySelectorAll(".vbox")].map((v) => readBoxState(v));
    const required =
      p3.boundary === 1
        ? ["north"]
        : p3.boundary === 3
          ? ["north", "south"]
          : p3.boundary === 5
            ? ["north", "south", "east", "west"]
            : ["north", "south", "east"];
    if (!boxes.length)
      return {
        html: `You still need <code class="tok-name">${required.join(
          '</code>, <code class="tok-name">',
        )}</code> in the program state.`,
      };

    const by = Object.fromEntries(boxes.map((b) => [b.name, b]));
    const coreRequired = p3.boundary >= 4 ? ["north", "south"] : required;
    const missingCore = coreRequired.filter((name) => !by[name]);
    if (missingCore.length)
      return {
        html: `You still need <code class="tok-name">${missingCore[0]}</code> in the program state.`,
      };
    if (p3.boundary >= 4) {
      const extras = boxes.filter((b) => !["north", "south"].includes(b.name));
      if (!extras.length)
        return {
          html: 'You still need to create a new variable for <code class="tok-name">east</code>.',
        };
      if (!extras.some((b) => b.name === "east"))
        return {
          html: 'The new variable\'s name should be <code class="tok-name">east</code>.',
        };
      if (p3.boundary === 5) {
        if (extras.length < 2)
          return {
            html: 'You still need to create a new variable for <code class="tok-name">west</code>.',
          };
        if (!extras.some((b) => b.name === "west"))
          return {
            html: 'The new variable\'s name should be <code class="tok-name">west</code>.',
          };
      }
    }
    const missing = required.filter((name) => !by[name]);
    if (missing.length)
      return {
        html: `You still need <code class="tok-name">${missing[0]}</code> in the program state.`,
      };
    if (boxes.length > required.length) {
      const extra = boxes.find((b) => !required.includes(b.name));
      return {
        html: `Only keep <code class="tok-name">${required.join('</code>, <code class="tok-name">')}</code> in the program state. Remove <code class="tok-name">${extra?.name || "the extra variable"}</code>.`,
      };
    }
    const wrongType = boxes.find((b) => b.type !== "int");
    if (wrongType) {
      const label = wrongType.name
        ? `<code class="tok-name">${wrongType.name}</code>`
        : "This variable";
      const rawType = String(wrongType.type || "").trim();
      const typeLabel = rawType
        ? `<code class="tok-type">${rawType}</code>`
        : "blank";
      return {
        html: `${label}\'s type should be <code class="tok-type">int</code>, not ${typeLabel}.`,
      };
    }

    if (p3.boundary === 1) {
      if (!isEmptyVal(by.north.value || ""))
        return {
          html: '<code class="tok-name">north</code> has not been assigned yet—leave its value empty.',
        };
    } else if (p3.boundary === 3) {
      if (by.north.value !== "5")
        return {
          html: 'Line 3 assigns <code class="tok-line">north = 5;</code>.',
        };
      if (by.south.value !== "-5")
        return {
          html: 'Line 2 assigns <code class="tok-line">south = -5;</code>.',
        };
    } else if (p3.boundary === 4) {
      if (by.north.value !== "5")
        return {
          html: '<code class="tok-name">north</code> keeps the value <code class="tok-value">5</code>.',
        };
      if (by.south.value !== "-5")
        return {
          html: '<code class="tok-name">south</code> keeps the value <code class="tok-value">-5</code>.',
        };
      if (by.east.value !== "9")
        return {
          html: 'Line 4 declares <code class="tok-name">east</code> and assigns <code class="tok-value">9</code>.',
        };
    } else if (p3.boundary === 5) {
      if (by.west?.value !== "-9")
        return {
          html: 'Line 5 declares <code class="tok-name">west</code> and assigns <code class="tok-value">-9</code>.',
        };
      if (by.east.value !== "9")
        return {
          html: '<code class="tok-name">east</code>\'s value should stay <code class="tok-value">9</code>.',
        };
      if (by.south.value !== "-5")
        return {
          html: '<code class="tok-name">south</code>\'s value should stay <code class="tok-value">-5</code>.',
        };
      if (by.north.value !== "5")
        return {
          html: '<code class="tok-name">north</code>\'s value should remain <code class="tok-value">5</code>.',
        };
    }
    const allTypesOk = boxes.every((b) => b.type === "int");
    const ok =
      (p3.boundary === 1 &&
        boxes.length === 1 &&
        allTypesOk &&
        by.north &&
        isEmptyVal(by.north.value || "")) ||
      (p3.boundary === 3 &&
        boxes.length === 2 &&
        allTypesOk &&
        by.north &&
        by.south &&
        by.north.value === "5" &&
        by.south.value === "-5") ||
      (p3.boundary === 4 &&
        boxes.length === 3 &&
        allTypesOk &&
        by.north &&
        by.south &&
        by.east &&
        by.north.value === "5" &&
        by.south.value === "-5" &&
        by.east.value === "9") ||
      (p3.boundary === 5 &&
        boxes.length === 4 &&
        allTypesOk &&
        by.north &&
        by.south &&
        by.east &&
        by.west &&
        by.north.value === "5" &&
        by.south.value === "-5" &&
        by.east.value === "9" &&
        by.west.value === "-9");
    if (ok)
      return { html: 'Looks good. Press <span class="btn-ref">Check</span>.' };
    const hasReset = !!document.getElementById("p3-reset");
    return hasReset
      ? {
          html: 'Your program has a problem that isn\'t covered by a hint. Try starting over on this step by clicking <span class="btn-ref">Reset</span>.',
        }
      : "Your program has a problem that isn't covered by a hint. Sorry.";
  }

  function renderCodePane3() {
    const progress =
      (p3.boundary === 1 && !p3.pass1) ||
      (p3.boundary === 3 && !p3.pass3) ||
      (p3.boundary === 5 && !p3.pass5);
    renderCodePane($("#p3-code"), p3.lines, p3.boundary, { progress });
  }

  function restoreDefaults(state, defaults) {
    return restoreWorkspace(state, defaults, "p3workspace");
  }

  function p3Render() {
    renderCodePane3();
    const stage = $("#p3-stage");
    stage.innerHTML = "";
    const instructions = $("#p3-instructions");
    const solved =
      (p3.boundary === 1 && p3.pass1) ||
      (p3.boundary === 3 && p3.pass3) ||
      (p3.boundary === 5 && p3.pass5);
    if (solved) {
      $("#p3-status").textContent = "correct";
      $("#p3-status").className = "ok";
    } else {
      $("#p3-status").textContent = "";
      $("#p3-status").className = "muted";
    }
    $("#p3-add").classList.add("hidden");
    $("#p3-check").classList.add("hidden");
    $("#p3-reset").classList.add("hidden");

    resetHint();
    if (instructions) {
      if (p3.boundary === 0) {
        instructions.textContent = "No instructions for this one. Good luck!";
        instructions.classList.remove("hidden");
      } else {
        instructions.textContent = "";
        instructions.classList.add("hidden");
      }
    }
    hint.setButtonHidden(true);

    if (p3.boundary === 0) {
      // blank
    } else if (p3.boundary === 1) {
      const editable = !p3.pass1;
      const wrap = restoreWorkspace(p3.ws1, [], "p3workspace", {
        editable,
        deletable: editable,
      });
      if (!editable)
        wrap.querySelectorAll(".vbox").forEach((v) => MB.disableBoxEditing(v));
      stage.appendChild(wrap);
      if (editable) {
        $("#p3-add").classList.remove("hidden");
        $("#p3-check").classList.remove("hidden");
        hint.setButtonHidden(false);
        ensureBaseline(1, []);
        attachResetWatcher(wrap, 1);
      }
    } else if (p3.boundary === 2) {
      const wrap = el('<div class="grid"></div>');
      const a = vbox({
        addr: String(p3.aAddr),
        type: "int",
        value: "empty",
        name: "north",
        editable: false,
      });
      const b = vbox({
        addr: String(p3.bAddr),
        type: "int",
        value: "-5",
        name: "south",
        editable: false,
      });
      a.querySelector(".value").classList.add("placeholder", "muted");
      wrap.appendChild(a);
      wrap.appendChild(b);
      stage.appendChild(wrap);
    } else if (p3.boundary === 3) {
      const editable = !p3.pass3;
      const wrap = restoreWorkspace(
        p3.ws3,
        [
          {
            name: "north",
            type: "int",
            value: "empty",
            address: String(p3.aAddr),
          },
          {
            name: "south",
            type: "int",
            value: "-5",
            address: String(p3.bAddr),
          },
        ],
        "p3workspace",
        { editable, deletable: editable },
      );
      if (!editable)
        wrap.querySelectorAll(".vbox").forEach((v) => MB.disableBoxEditing(v));
      stage.appendChild(wrap);
      if (editable) {
        $("#p3-add").classList.remove("hidden");
        $("#p3-check").classList.remove("hidden");
        hint.setButtonHidden(false);
        ensureBaseline(3, [
          {
            name: "north",
            type: "int",
            value: "empty",
            address: String(p3.aAddr),
          },
          {
            name: "south",
            type: "int",
            value: "-5",
            address: String(p3.bAddr),
          },
        ]);
        attachResetWatcher(wrap, 3);
      }
    } else if (p3.boundary === 4) {
      const wrap = el('<div class="grid"></div>');
      const north = vbox({
        addr: String(p3.aAddr),
        type: "int",
        value: "5",
        name: "north",
        editable: false,
      });
      const south = vbox({
        addr: String(p3.bAddr),
        type: "int",
        value: "-5",
        name: "south",
        editable: false,
      });
      const east = vbox({
        addr: String(p3.cAddr),
        type: "int",
        value: "9",
        name: "east",
        editable: false,
      });
      wrap.appendChild(north);
      wrap.appendChild(south);
      wrap.appendChild(east);
      stage.appendChild(wrap);
    } else if (p3.boundary === 5) {
      const defaults = (() => {
        const base = cloneStateBoxes(p3.ws3);
        if (!base.length) {
          base.push(
            {
              name: "north",
              type: "int",
              value: "5",
              address: String(p3.aAddr),
            },
            {
              name: "south",
              type: "int",
              value: "-5",
              address: String(p3.bAddr),
            },
            {
              name: "east",
              type: "int",
              value: "9",
              address: String(p3.cAddr),
            },
          );
        }
        ensureBox(base, {
          name: "north",
          type: "int",
          value: "5",
          address: String(p3.aAddr),
        });
        ensureBox(base, {
          name: "south",
          type: "int",
          value: "-5",
          address: String(p3.bAddr),
        });
        ensureBox(base, {
          name: "east",
          type: "int",
          value: "9",
          address: String(p3.cAddr),
        });
        return base;
      })();
      const editable = !p3.pass5;
      const wrap = restoreWorkspace(p3.ws5, defaults, "p3workspace", {
        editable,
        deletable: editable,
      });
      if (!editable)
        wrap.querySelectorAll(".vbox").forEach((v) => MB.disableBoxEditing(v));
      stage.appendChild(wrap);
      if (editable) {
        $("#p3-add").classList.remove("hidden");
        $("#p3-check").classList.remove("hidden");
        hint.setButtonHidden(false);
        ensureBaseline(5, defaults);
        attachResetWatcher(wrap, 5);
      }
    }
  }

  function saveWorkspace() {
    if (p3.boundary === 1) p3.ws1 = serializeWorkspace("p3workspace");
    if (p3.boundary === 3) p3.ws3 = serializeWorkspace("p3workspace");
    if (p3.boundary === 5) p3.ws5 = serializeWorkspace("p3workspace");
  }

  $("#p3-add").onclick = () => {
    const ws = document.getElementById("p3workspace");
    if (ws) ws.appendChild(makeAnswerBox({}));
    updateResetVisibility(p3.boundary);
  };

  $("#p3-reset").onclick = () => {
    if (p3.boundary === 1) p3.ws1 = null;
    if (p3.boundary === 3) p3.ws3 = null;
    if (p3.boundary === 5) p3.ws5 = null;
    p3Render();
  };

  $("#p3-check").onclick = () => {
    resetHint();
    const ws = $("#p3workspace");
    if (!ws) return;
    const boxes = [...ws.querySelectorAll(".vbox")].map((v) => readBoxState(v));
    const by = Object.fromEntries(boxes.map((b) => [b.name, b]));
    const allTypesOk = boxes.every((b) => b.type === "int");

    if (p3.boundary === 1) {
      const ok =
        boxes.length === 1 &&
        allTypesOk &&
        by.north &&
        isEmptyVal(by.north.value || "");
      $("#p3-status").textContent = ok ? "correct" : "incorrect";
      $("#p3-status").className = ok ? "ok" : "err";
      MB.flashStatus($("#p3-status"));
      if (ok) {
        ws.querySelectorAll(".vbox").forEach((v) => MB.disableBoxEditing(v));
        MB.removeBoxDeleteButtons(ws);
        $("#p3-check").classList.add("hidden");
        $("#p3-add").classList.add("hidden");
        $("#p3-reset").classList.add("hidden");
        hint.hide();
        $("#p3-hint-btn")?.classList.add("hidden");
        p3.pass1 = true;
        renderCodePane3();
        MB.pulseNextButton("p3");
        pager.update();
      }
      return;
    }

    if (p3.boundary === 3) {
      const ok =
        boxes.length === 2 &&
        allTypesOk &&
        by.north &&
        by.south &&
        by.north.value === "5" &&
        by.south.value === "-5";
      $("#p3-status").textContent = ok ? "correct" : "incorrect";
      $("#p3-status").className = ok ? "ok" : "err";
      MB.flashStatus($("#p3-status"));
      if (ok) {
        ws.querySelectorAll(".vbox").forEach((v) => MB.disableBoxEditing(v));
        MB.removeBoxDeleteButtons(ws);
        $("#p3-check").classList.add("hidden");
        $("#p3-add").classList.add("hidden");
        $("#p3-reset").classList.add("hidden");
        hint.hide();
        $("#p3-hint-btn")?.classList.add("hidden");
        p3.pass3 = true;
        renderCodePane3();
        MB.pulseNextButton("p3");
        pager.update();
      }
      return;
    }

    if (p3.boundary === 5) {
      const ok =
        boxes.length === 4 &&
        allTypesOk &&
        by.north &&
        by.south &&
        by.east &&
        by.west &&
        by.north.value === "5" &&
        by.south.value === "-5" &&
        by.east.value === "9" &&
        by.west.value === "-9";
      $("#p3-status").textContent = ok ? "correct" : "incorrect";
      $("#p3-status").className = ok ? "ok" : "err";
      MB.flashStatus($("#p3-status"));
      if (ok) {
        ws.querySelectorAll(".vbox").forEach((v) => MB.disableBoxEditing(v));
        MB.removeBoxDeleteButtons(ws);
        $("#p3-check").classList.add("hidden");
        $("#p3-add").classList.add("hidden");
        $("#p3-reset").classList.add("hidden");
        hint.hide();
        $("#p3-hint-btn")?.classList.add("hidden");
        p3.pass5 = true;
        renderCodePane3();
        MB.pulseNextButton("p3");
        pager.update();
      }
    }
  };

  const pager = createStepper({
    prefix: "p3",
    lines: p3.lines,
    nextPage: NEXT_PAGE,
    getBoundary: () => p3.boundary,
    setBoundary: (val) => {
      p3.boundary = val;
    },
    onBeforeChange: saveWorkspace,
    onAfterChange: p3Render,
    isStepLocked: (boundary) => {
      if (boundary === 1) return !p3.pass1;
      if (boundary === 3) return !p3.pass3;
      if (boundary === 5) return !p3.pass5;
      return false;
    },
    getStepBadge: (step) => {
      if (step === 1) return p3.pass1 ? "check" : "note";
      if (step === 3) return p3.pass3 ? "check" : "note";
      if (step === 5) return p3.pass5 ? "check" : "note";
      return "";
    },
  });

  p3Render();
  pager.update();
})(window.MB);
