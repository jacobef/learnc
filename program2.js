(function (MB) {
  const {
    $,
    randAddr,
    renderCodePane,
    vbox,
    readBoxState,
    isEmptyVal,
    createHintController,
    createStepper,
    flashStatus,
    pulseNextButton,
    disableBoxEditing,
    removeBoxDeleteButtons,
    restoreWorkspace,
    serializeWorkspace,
    makeAnswerBox,
  } = MB;

  const instructions = $("#p2-instructions");
  const NEXT_PAGE = "program3.html";

  const p2 = {
    lines: ["int toaster;", "int fridge;", "toaster = 28;"],
    boundary: 0,
    toasterAddr: randAddr("int"),
    fridgeAddr: randAddr("int"),
    stateGhi: null,
    wsGhi: null,
    stateAssign: null,
    passGhi: false,
    passAssign: false,
  };

  const hint = createHintController({
    button: "#p2-hint-btn",
    panel: "#p2-hint",
    build: buildHint,
  });

  function resetHint() {
    hint.hide();
  }

  function isGhiCorrect(box) {
    return (
      box &&
      box.name === "fridge" &&
      box.type === "int" &&
      isEmptyVal(box.value || "")
    );
  }

  function isAssignCorrect(toasterBox, fridgeBox) {
    return (
      toasterBox &&
      toasterBox.name === "toaster" &&
      toasterBox.type === "int" &&
      toasterBox.value === "28" &&
      (!fridgeBox || isEmptyVal(fridgeBox.value || ""))
    );
  }

  function buildHint() {
    const boxes = [...document.querySelectorAll("#p2-stage .vbox")].map((v) =>
      readBoxState(v),
    );
    if (p2.boundary === 2) {
      if (boxes.length < 2) return { html: "Read the instructions." };
      const fridge =
        boxes.find((b) => b.name === "fridge") ||
        boxes.find((b) => b.name && b.name !== "toaster") ||
        boxes.find((b) => b);
      if (!fridge)
        return "Your program has a problem that isn't covered by a hint. Sorry.";
      if (!fridge.name)
        return {
          html: 'The new variable\'s name should be <code class="tok-name">fridge</code>.',
        };
      if (fridge.name !== "fridge")
        return {
          html: 'The new variable\'s name should be <code class="tok-name">fridge</code>.',
        };
      if (fridge.type !== "int")
        return {
          html: '<code class="tok-name">fridge</code>\'s type should be <code class="tok-type">int</code>.',
        };
      if (!isEmptyVal(fridge.value || ""))
        return {
          html: "Right after declaration the value should still be empty.",
        };
      if (isGhiCorrect(fridge))
        return {
          html: 'Looks good. Press <span class="btn-ref">Check</span>.',
        };
      return "Your program has a problem that isn't covered by a hint. Sorry.";
    }
    if (p2.boundary === 3) {
      if (boxes.length < 2) return { html: "Read the instructions." };
      const toaster = boxes.find((b) => b.name === "toaster") || boxes[0];
      const fridge = boxes.find((b) => b.name === "fridge");
      if (!toaster)
        return "Your program has a problem that isn't covered by a hint. Sorry.";
      if (toaster.name !== "toaster")
        return {
          html: 'The variable here is <code class="tok-name">toaster</code>, so the name should read <code class="tok-name">toaster</code>.',
        };
      if (toaster.type !== "int")
        return {
          html: '<code class="tok-name">toaster</code>\'s type should be <code class="tok-type">int</code>.',
        };
      if (fridge && fridge.value === "28" && toaster.value !== "28")
        return {
          html: '<code class="tok-value">28</code> belongs in <code class="tok-name">toaster</code>\'s value, not <code class="tok-name">fridge</code>.',
        };
      if (fridge && !isEmptyVal(fridge.value || ""))
        return {
          html: 'This line doesn\'t change <code class="tok-name">fridge</code>. Leave its value empty.',
        };
      if (isEmptyVal(toaster.value || ""))
        return {
          html: 'Set <code class="tok-name">toaster</code>\'s value to <code class="tok-value">28</code>.',
        };
      if (toaster.value !== "28")
        return {
          html: 'Line 3 assigns <code class="tok-line">toaster = 28;</code>.',
        };
      if (isAssignCorrect(toaster, fridge))
        return {
          html: 'Looks good. Press <span class="btn-ref">Check</span>.',
        };
      return "Your program has a problem that isn't covered by a hint. Sorry.";
    }
    return "Your program has a problem that isn't covered by a hint. Sorry.";
  }

  function updateInstructions() {
    if (!instructions) return;
    if (p2.boundary === p2.lines.length && p2.passAssign) {
      instructions.textContent = "Program solved!";
      return;
    }
    if (p2.boundary === 2) {
      instructions.innerHTML =
        '<code class="tok-line">int fridge;</code> creates a new variable. Click <span class="btn-ref">+ New variable</span> and enter its attributes.';
    } else if (p2.boundary === 3) {
      instructions.innerHTML =
        'What does <code class="tok-line">toaster = 28;</code> do?';
    } else {
      const runLabel = `Run line ${p2.boundary + 1} ▶`;
      if (p2.boundary <= 0) {
        instructions.innerHTML = `Use <span class="btn-ref">${runLabel}</span> to step through the code.`;
      } else {
        instructions.innerHTML = `Use <span class="btn-ref">Back ◀</span> and <span class="btn-ref">${runLabel}</span> to step through the code.`;
      }
    }
  }

  function setStatus(text, cls = "muted") {
    const status = $("#p2-status");
    if (!status) return;
    status.textContent = text;
    status.className = cls;
  }

  function renderCodePane2() {
    const progress =
      (p2.boundary === 2 && !p2.passGhi) ||
      (p2.boundary === 3 && !p2.passAssign);
    renderCodePane($("#p2-code"), p2.lines, p2.boundary, { progress });
  }

  function addPlaceholderIfEmpty(box) {
    if (!box) return;
    const valEl = box.querySelector(".value");
    if (valEl && isEmptyVal(valEl.textContent || "")) {
      valEl.classList.add("placeholder", "muted");
    }
  }

  function p2Render() {
    renderCodePane2();
    const stage = $("#p2-stage");
    stage.innerHTML = "";
    resetHint();
    hint.setButtonHidden(true);
    $("#p2-check").classList.add("hidden");
    $("#p2-add").classList.add("hidden");

    const solved =
      (p2.boundary === 2 && p2.passGhi) || (p2.boundary === 3 && p2.passAssign);
    if (solved) {
      setStatus("correct", "ok");
    } else {
      setStatus("", "muted");
    }

    updateInstructions();

    if (p2.boundary === 0) {
      return;
    }

    const wrap = document.createElement("div");
    wrap.className = "grid";

    if (p2.boundary === 1 || p2.boundary === 2) {
      const toasterStatic = vbox({
        addr: String(p2.toasterAddr),
        type: "int",
        value: "empty",
        name: "toaster",
        editable: false,
      });
      addPlaceholderIfEmpty(toasterStatic);
      wrap.appendChild(toasterStatic);
    }

    if (p2.boundary === 1) {
      stage.appendChild(wrap);
      return;
    }

    if (p2.boundary === 2) {
      if (p2.fridgeAddr == null) p2.fridgeAddr = randAddr("int");
      const editable = !p2.passGhi;
      const defaults = [];
      if (p2.stateGhi) {
        defaults.push({
          name: p2.stateGhi.name || "",
          type: p2.stateGhi.type || "",
          value: p2.stateGhi.value ?? "empty",
          address:
            p2.stateGhi.addr ?? p2.stateGhi.address ?? String(p2.fridgeAddr),
        });
      }
      const workspace = restoreWorkspace(p2.wsGhi, defaults, "p2workspace", {
        editable,
        deletable: editable,
        allowNameEdit: true,
        allowTypeEdit: true,
      });
      MB.setupInlineWorkspace?.(workspace, wrap);
      wrap.appendChild(workspace);
      stage.appendChild(wrap);
      if (!editable) {
        wrap.querySelectorAll(".vbox").forEach((v) => disableBoxEditing(v));
      }
      if (editable) {
        $("#p2-check").classList.remove("hidden");
        $("#p2-add").classList.remove("hidden");
        hint.setButtonHidden(false);
      }
      return;
    }

    if (p2.boundary === 3) {
      // toaster for assignment
      const seedDef = p2.stateAssign ?? {
        addr: String(p2.toasterAddr),
        type: "int",
        value: "empty",
        name: "toaster",
      };
      const editable = !p2.passAssign;
      const toasterBox = vbox({
        addr: String(p2.toasterAddr),
        type: seedDef.type || "int",
        value: seedDef.value ?? "empty",
        name: "toaster",
        names: seedDef.names,
        editable,
        allowNameEdit: false,
        allowTypeEdit: false,
      });
      addPlaceholderIfEmpty(toasterBox);
      wrap.appendChild(toasterBox);

      // carry over fridge for context
      const fridgeSeed = p2.stateGhi ?? {
        addr: String(p2.fridgeAddr ?? randAddr("int")),
        type: "int",
        value: "empty",
        name: "fridge",
      };
      const fridge = vbox({
        addr: String(p2.fridgeAddr ?? fridgeSeed.addr),
        type: fridgeSeed.type || "int",
        value: fridgeSeed.value ?? "empty",
        name: fridgeSeed.name || "fridge",
        names: fridgeSeed.names,
        editable,
      });
      addPlaceholderIfEmpty(fridge);
      wrap.appendChild(fridge);

      stage.appendChild(wrap);
      if (!editable) {
        disableBoxEditing(toasterBox);
      } else {
        hint.setButtonHidden(false);
        $("#p2-check").classList.remove("hidden");
      }
    }
  }

  function p2Save() {
    if (p2.boundary === 2) {
      const boxes = [...document.querySelectorAll("#p2-stage .vbox")].map((v) =>
        readBoxState(v),
      );
      const ws = serializeWorkspace("p2workspace");
      if (Array.isArray(ws)) p2.wsGhi = ws;
      const fridge =
        boxes.find((b) => b.name === "fridge") ||
        boxes.find((b) => b.name && b.name !== "toaster");
      p2.stateGhi = fridge ? { ...fridge, addr: String(p2.fridgeAddr) } : null;
      return;
    }
    const boxes = [...document.querySelectorAll("#p2-stage .vbox")].map((v) =>
      readBoxState(v),
    );
    if (!boxes.length) return;
    if (p2.boundary === 3) {
      const toaster = boxes.find((b) => b.name === "toaster");
      if (toaster) {
        p2.stateAssign = { ...toaster, addr: String(p2.toasterAddr) };
      }
    }
  }

  $("#p2-check").onclick = () => {
    resetHint();
    const boxes = [...document.querySelectorAll("#p2-stage .vbox")].map((v) =>
      readBoxState(v),
    );

    if (p2.boundary === 2) {
      const fridge =
        boxes.find((b) => b.name === "fridge") ||
        boxes.find((b) => b.name && b.name !== "toaster");
      const ok = isGhiCorrect(fridge);
      setStatus(ok ? "correct" : "incorrect", ok ? "ok" : "err");
      flashStatus($("#p2-status"));
      if (ok) {
        document
          .querySelectorAll("#p2-stage .vbox")
          .forEach((v) => disableBoxEditing(v));
        removeBoxDeleteButtons(document.getElementById("p2-stage"));
        p2.passGhi = true;
        p2.stateGhi = { ...fridge, addr: String(p2.fridgeAddr) };
        const ws = serializeWorkspace("p2workspace");
        if (Array.isArray(ws)) p2.wsGhi = ws;
        renderCodePane2();
        $("#p2-check").classList.add("hidden");
        $("#p2-add").classList.add("hidden");
        hint.hide();
        $("#p2-hint-btn")?.classList.add("hidden");
        pulseNextButton("p2");
        pager.update();
      }
      return;
    }

    if (p2.boundary === 3) {
      const toaster = boxes.find((b) => b.name === "toaster");
      const fridge = boxes.find((b) => b.name === "fridge");
      const ok = isAssignCorrect(toaster, fridge);
      setStatus(ok ? "correct" : "incorrect", ok ? "ok" : "err");
      flashStatus($("#p2-status"));
      if (ok) {
        document
          .querySelectorAll("#p2-stage .vbox")
          .forEach((v) => disableBoxEditing(v));
        removeBoxDeleteButtons(document.getElementById("p2-stage"));
        p2.passAssign = true;
        p2.stateAssign = { ...toaster, addr: String(p2.toasterAddr) };
        renderCodePane2();
        $("#p2-check").classList.add("hidden");
        hint.hide();
        $("#p2-hint-btn")?.classList.add("hidden");
        pulseNextButton("p2");
        updateInstructions();
        pager.update();
      }
    }
  };

  $("#p2-add").onclick = () => {
    const ws = document.getElementById("p2workspace");
    if (!ws) return;
    ws.appendChild(makeAnswerBox({ allowNameEdit: true, allowTypeEdit: true }));
  };

  const pager = createStepper({
    prefix: "p2",
    lines: p2.lines,
    nextPage: NEXT_PAGE,
    getBoundary: () => p2.boundary,
    setBoundary: (val) => {
      p2.boundary = val;
    },
    onBeforeChange: p2Save,
    onAfterChange: () => {
      p2Render();
      updateInstructions();
    },
    isStepLocked: (boundary) => {
      if (boundary === 2) return !p2.passGhi;
      if (boundary === 3) return !p2.passAssign;
      return false;
    },
    getStepBadge: (step) => {
      if (step === 2) return p2.passGhi ? "check" : "note";
      if (step === 3) return p2.passAssign ? "check" : "note";
      return "";
    },
  });

  p2Render();
  updateInstructions();
  pager.update();
})(window.MB);
