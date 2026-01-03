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
    makeAnswerBox,
    createHintController,
    createStepper,
    disableAutoText,
    prependTopStepperNotice,
  } = MB;

  const instructions = $("#p8-instructions");
  const NEXT_PAGE = "dereferencing-quiz.html";

  const p8 = {
    lines: [
      "int deer;",
      "int hare;",
      "int* wolf;",
      "wolf = &deer;",
      "wolf = &hare;",
      "int** bear = &wolf;",
      "int* fox = wolf;",
      "deer = 50;",
      "*wolf = 11;",
      "*bear = &deer;",
      "int elk = *wolf;",
      "*fox = **bear;",
    ],
    boundary: 0,
    aAddr: randAddr("int"),
    bAddr: randAddr("int"),
    ptrAddr: randAddr("int*"),
    pptrAddr: randAddr("int**"),
    spareAddr: randAddr("int*"),
    pulledAddr: randAddr("int"),
    ws: Array(13).fill(null),
    passes: Array(13).fill(false),
    aliasExplained: false,
    baseline: Array(13).fill(null),
  };

  const editableSteps = new Set([7, 9, 10, 11, 12]);

  function ptrTarget(boundary) {
    if (boundary < 4) return "empty";
    if (boundary < 5) return String(p8.aAddr);
    if (boundary < 10) return String(p8.bAddr);
    return String(p8.aAddr);
  }

  function pptrTarget(boundary) {
    if (boundary < 6) return "empty";
    return String(p8.ptrAddr);
  }

  function canonical(boundary) {
    const state = [];
    const deerVal = boundary >= 8 ? "50" : "empty";
    const hareVal = boundary >= 12 ? "50" : boundary >= 9 ? "11" : "empty";
    if (boundary >= 1) {
      state.push({
        name: "deer",
        names: ["deer"],
        type: "int",
        value: deerVal,
        address: String(p8.aAddr),
      });
    }
    if (boundary >= 2) {
      state.push({
        name: "hare",
        names: ["hare"],
        type: "int",
        value: hareVal,
        address: String(p8.bAddr),
      });
    }
    if (boundary >= 3) {
      state.push({
        name: "wolf",
        names: ["wolf"],
        type: "int*",
        value: ptrTarget(boundary),
        address: String(p8.ptrAddr),
      });
      const targetVal = ptrTarget(boundary);
      if (targetVal !== "empty") {
        const tgt = state.find((b) => b.address === String(targetVal));
        if (tgt) {
          const names = tgt.names || [tgt.name];
          if (!names.includes("*wolf")) names.push("*wolf");
          tgt.names = names;
        }
      }
    }
    if (boundary >= 6) {
      state.push({
        name: "bear",
        names: ["bear"],
        type: "int**",
        value: pptrTarget(boundary),
        address: String(p8.pptrAddr),
      });
      const pptrVal = pptrTarget(boundary);
      if (pptrVal !== "empty") {
        const ptrBox = state.find((b) => b.address === String(pptrVal));
        if (ptrBox) {
          const ptrAlias = "*bear";
          const ptrNames = ptrBox.names || [ptrBox.name];
          if (!ptrNames.includes(ptrAlias)) ptrNames.push(ptrAlias);
          ptrBox.names = ptrNames;
        }
        const ptrTargetAddr = ptrTarget(boundary);
        const targetBox = state.find(
          (b) => b.address === String(ptrTargetAddr),
        );
        if (targetBox) {
          const derefAlias = "**bear";
          const names = targetBox.names || [targetBox.name];
          if (!names.includes(derefAlias)) names.push(derefAlias);
          targetBox.names = names;
        }
      }
    }
    if (boundary >= 7) {
      state.push({
        name: "fox",
        names: ["fox"],
        type: "int*",
        value: String(p8.bAddr),
        address: String(p8.spareAddr),
      });
      const foxTarget = String(p8.bAddr);
      if (foxTarget !== "empty") {
        const tgt = state.find((b) => b.address === String(foxTarget));
        if (tgt) {
          const names = tgt.names || [tgt.name];
          if (!names.includes("*fox")) names.push("*fox");
          tgt.names = names;
        }
      }
    }
    if (boundary >= 11) {
      state.push({
        name: "elk",
        names: ["elk"],
        type: "int",
        value: state.find((b) => b.name === "deer")?.value || "empty",
        address: String(p8.pulledAddr),
      });
    }
    return state;
  }

  function normalizePtrValue(value) {
    if (!value) return "empty";
    const trimmed = value.trim();
    if (!trimmed || /^empty$/i.test(trimmed)) return "empty";
    return trimmed;
  }

  function applyAliasNames(boxes) {
    const next = boxes.map((b) => ({
      ...b,
      names: b.name ? [b.name] : [],
    }));
    const byAddr = new Map();
    const byName = new Map();
    next.forEach((b) => {
      const addr = b.addr ?? b.address;
      if (addr != null && addr !== "") byAddr.set(String(addr), b);
      if (b.name) byName.set(b.name, b);
    });
    const addAlias = (box, alias) => {
      if (!box) return;
      if (!box.names) box.names = [];
      if (!box.names.includes(alias)) box.names.push(alias);
    };
    const wolf = byName.get("wolf");
    const bear = byName.get("bear");
    const fox = byName.get("fox");
    const wolfVal = normalizePtrValue(wolf?.value || "");
    if (wolf && wolfVal !== "empty") {
      addAlias(byAddr.get(String(wolfVal)), "*wolf");
    }
    const foxVal = normalizePtrValue(fox?.value || "");
    if (fox && foxVal !== "empty") {
      addAlias(byAddr.get(String(foxVal)), "*fox");
    }
    const bearVal = normalizePtrValue(bear?.value || "");
    if (bear && bearVal !== "empty") {
      addAlias(byAddr.get(String(bearVal)), "*bear");
      if (wolf && wolfVal !== "empty") {
        addAlias(byAddr.get(String(wolfVal)), "**bear");
      }
    }
    return next;
  }

  function rebuildNameList(boxEl, namesList) {
    const list = boxEl.querySelector(".name-list");
    const inner = boxEl.querySelector(".name-list-inner");
    const label = boxEl.querySelector(".lbl-name");
    if (!list || !inner || !label) return;
    const keepEditable = !!boxEl.querySelector(".name-text[contenteditable]");
    const editable = boxEl.classList.contains("is-editable");
    const nameClasses = `name-tag${editable ? " editable" : ""}`;
    const wasExpanded = list.classList.contains("expanded");
    const canToggle = namesList.length > 1;
    list.className = `name-list${canToggle ? " collapsible" : ""}${wasExpanded && canToggle ? " expanded" : ""}`;
    const nameTags = namesList
      .map((n, idx) => {
        const extraClass = canToggle && idx > 0 ? " name-extra" : "";
        const cls =
          namesList.length > 1
            ? `${nameClasses}${extraClass}`
            : `${nameClasses} single`;
        return `<span class="${cls}"><span class="name-text">${n}</span></span>`;
      })
      .join("");
    const toggleBtn = canToggle
      ? `<button class="name-toggle" type="button" aria-expanded="${wasExpanded ? "true" : "false"}">${wasExpanded ? "Hide other names" : "Other names"}</button>`
      : "";
    inner.innerHTML = `${nameTags}${toggleBtn}`;
    if (keepEditable) {
      inner.querySelectorAll(".name-text").forEach((el) => {
        el.setAttribute("contenteditable", "true");
        el.classList.add("editable");
        disableAutoText?.(el);
      });
    }
    label.textContent = namesList.length > 1 ? "name(s)" : "name";
    if (canToggle) {
      const toggle = inner.querySelector(".name-toggle");
      if (toggle) {
        const setExpanded = (expanded) => {
          list.classList.toggle("expanded", expanded);
          toggle.setAttribute("aria-expanded", expanded ? "true" : "false");
          toggle.textContent = expanded ? "Hide other names" : "Other names";
        };
        toggle.onclick = () =>
          setExpanded(!list.classList.contains("expanded"));
      }
    }
  }

  function refreshAliasNames(wrap) {
    if (!wrap) return;
    if (wrap.querySelector(".name-text[contenteditable]:focus")) return;
    const boxes = [...wrap.querySelectorAll(".vbox")].map((v) =>
      readBoxState(v),
    );
    const normalized = applyAliasNames(boxes);
    const byName = new Map();
    normalized.forEach((b) => {
      if (b.name) byName.set(String(b.name), b);
    });
    wrap.querySelectorAll(".vbox").forEach((v) => {
      const base = readBoxState(v)?.name || "";
      const next = byName.get(base);
      if (!next || !Array.isArray(next.names)) return;
      rebuildNameList(v, next.names);
    });
  }

  function carriedState(boundary) {
    for (let b = boundary - 1; b >= 0; b--) {
      const st = p8.ws[b];
      if (Array.isArray(st) && st.length) return cloneBoxes(st);
    }
    return null;
  }

  function defaultsFor(boundary) {
    if (!editableSteps.has(boundary)) return canonical(boundary);
    const carried = carriedState(boundary);
    if (carried) return carried;
    const prev = canonical(boundary - 1);
    return cloneBoxes(prev);
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

  function updateStatus() {
    if (p8.boundary === p8.lines.length && p8.passes[p8.lines.length]) {
      setInstructions("Program solved!");
      return;
    }
    if (p8.boundary === 0) {
      setInstructions(
        prependTopStepperNotice(
          "p8",
          'Let’s revisit 7, but with some lines added at the end. To understand these new lines, we need a better understanding of what was going on in 7. Click <span class="btn-ref">Run line 1 ▶</span> to continue.',
          { html: true },
        ),
        { html: true },
      );
      return;
    }
    if (p8.boundary === 4) {
      setInstructions(
        'When <code class="tok-name">wolf</code> is assigned to <code class="tok-addr">&amp;deer</code>, <code class="tok-name">wolf</code>&#8217;s value becomes <code class="tok-name">deer</code>&#8217;s address. Also, <code class="tok-name">deer</code> gains an additional name. Use the <span class="btn-ref">Other names</span> toggle under <code class="tok-name">deer</code> to reveal this name.<br><br>We say that <code class="tok-name">wolf</code> now "points to" <code class="tok-name">deer</code>.',
        { html: true },
      );
      return;
    }
    if (p8.boundary === 5) {
      setInstructions(
        'When <code class="tok-name">wolf</code> is assigned to <code class="tok-addr">&amp;hare</code>, the <code class="tok-name">*wolf</code> name moves from <code class="tok-name">deer</code> to <code class="tok-name">hare</code>. Use the <span class="btn-ref">Other names</span> toggle under <code class="tok-name">hare</code> to reveal it. We say that <code class="tok-name">wolf</code> now "points to" <code class="tok-name">hare</code>.<br><br>In general, if some variable <code class="tok-name">X</code> points to another variable <code class="tok-name">Y</code>, then <code class="tok-name">*X</code> refers to <code class="tok-name">Y</code>. In this case, <code class="tok-name">wolf</code> points to <code class="tok-name">hare</code>, so <code class="tok-name">*wolf</code> refers to <code class="tok-name">hare</code>.<br><br>We&#8217;ll see the relevance of this alternate name later in the code.',
        { html: true },
      );
      return;
    }
    if (p8.boundary === 6) {
      setInstructions(
        '<code class="tok-line">bear = &amp;wolf;</code> adds the <code class="tok-name">*bear</code> name to <code class="tok-name">wolf</code>. It also adds a name to <code class="tok-name">hare</code>; use the <span class="btn-ref">Other names</span> toggle under <code class="tok-name">hare</code> to reveal it.<br><br>To understand why: as we just established, <code class="tok-name">*bear</code> now refers to <code class="tok-name">wolf</code>. <code class="tok-name">wolf</code> points to <code class="tok-name">hare</code>. So, <code class="tok-name">*bear</code> points to <code class="tok-name">hare</code>. Therefore, we can refer to <code class="tok-name">hare</code> by adding another asterisk to <code class="tok-name">*bear</code>, to get <code class="tok-name">**bear</code>.',
        { html: true },
      );
      return;
    }
    if (p8.boundary === 7) {
      setInstructions(
        "You're on your own now, good luck! The \"Other names\" will update automatically as you make edits.",
      );
      return;
    }
    setInstructions("");
  }

  function puzzleComplete() {
    return !!p8.passes[p8.lines.length];
  }

  const hint = createHintController({
    button: "#p8-hint-btn",
    panel: "#p8-hint",
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
    const refresh = () => {
      updateResetVisibility(boundary);
      refreshAliasNames(wrap);
    };
    wrap.addEventListener("input", refresh);
    wrap.addEventListener("click", () => setTimeout(refresh, 0));
    refresh();
  }

  function buildHint() {
    const ws = document.getElementById("p8workspace");
    if (!ws)
      return {
        html: 'Use <span class="btn-ref">Run line 1 ▶</span> to begin editing.',
      };
    const boxes = [...ws.querySelectorAll(".vbox")].map((v) => readBoxState(v));
    const expected = canonical(p8.boundary);
    const byName = (name) =>
      boxes.find((b) => b.name === name || (b.names || []).includes(name));

    const filledInt = ["deer", "hare"]
      .map((name) => byName(name))
      .find((box) => box && !isEmptyVal(box.value || ""));
    if (filledInt && p8.boundary < 8)
      return {
        html: `<code class="tok-name">${filledInt.name}</code> hasn't stored a value—leave it empty.`,
      };

    const wolf = byName("wolf");
    const bear = byName("bear");
    const fox = byName("fox");

    if (p8.boundary === 7) {
      const normalized = bear ? normalizePtrValue(bear.value || "") : "empty";
      if (normalized === "empty" || normalized !== String(p8.ptrAddr))
        return {
          html: '<code class="tok-name">bear</code>\'s value should be set to <code class="tok-name">wolf</code>\'s address.',
        };
      if (!fox)
        return {
          html: 'You need <code class="tok-name">fox</code> in the program state.',
        };
      const spareVal = fox ? normalizePtrValue(fox.value || "") : "empty";
      const expectedPtr = wolf ? normalizePtrValue(wolf.value || "") : "empty";
      if (fox && spareVal !== expectedPtr)
        return {
          html: '<code class="tok-name">fox</code>\'s value should be set to <code class="tok-name">wolf</code>\'s value (the address of <code class="tok-name">hare</code>).',
        };
    } else if (p8.boundary === 9) {
      const wolfVal = wolf ? normalizePtrValue(wolf.value || "") : "empty";
      const hareBox = byName("hare");
      if (wolfVal === "11" && hareBox?.value !== "11")
        return {
          html: '<code class="tok-line">*wolf = 11;</code> assigns through <code class="tok-name">wolf</code>, so it should change <code class="tok-name">hare</code>, not <code class="tok-name">wolf</code>.',
        };
      const bBox = byName("hare");
      if (bBox && bBox.value !== "11")
        return {
          html: '<code class="tok-name">*wolf</code> and <code class="tok-name">hare</code> are both names for the same variable, so <code class="tok-line">*wolf = 11;</code> would be equivalent to <code class="tok-line">hare = 11;</code>.',
        };
    } else if (p8.boundary === 10) {
      const deerAddr = String(p8.aAddr);
      const bearVal = bear ? normalizePtrValue(bear.value || "") : "empty";
      const wolfVal = wolf ? normalizePtrValue(wolf.value || "") : "empty";
      if (bearVal === deerAddr && wolfVal !== deerAddr)
        return {
          html: '<code class="tok-line">*bear = &amp;deer;</code> assigns through <code class="tok-name">bear</code>, so it should change <code class="tok-name">wolf</code>, not <code class="tok-name">bear</code>.',
        };
      const normalized = wolf ? normalizePtrValue(wolf.value || "") : "empty";
      if (normalized === "empty")
        return {
          html: 'Set <code class="tok-name">wolf</code> to <code class="tok-addr">&amp;deer</code> with <code class="tok-line">*bear = &amp;deer;</code>.',
        };
      if (normalized !== String(p8.aAddr))
        return {
          html: '<code class="tok-line">*bear = &amp;deer;</code> sets <code class="tok-name">wolf</code> to <code class="tok-addr">&amp;deer</code>.',
        };
    } else if (p8.boundary === 11) {
      const elk = byName("elk");
      const bBox = byName("deer");
      const bVal = bBox?.value || "";
      if (!elk)
        return {
          html: 'Make sure <code class="tok-name">elk</code> exists for this line.',
        };
      if (elk.value !== bVal)
        return {
          html: '<code class="tok-name">elk</code>\'s value should be set to <code class="tok-name">*wolf</code>\'s value.',
        };
    } else if (p8.boundary === 12) {
      const hareBox = byName("hare");
      const deerBox = byName("deer");
      if (!hareBox || !deerBox)
        return {
          html: "Make sure both hare and deer exist for this line.",
        };
      if (hareBox.value !== deerBox.value)
        return {
          html: '<code class="tok-line">*fox = **bear;</code> assigns through <code class="tok-name">fox</code>, so <code class="tok-name">hare</code> should match <code class="tok-name">**bear</code>.',
        };
    }
    const verdict = validateWorkspace(p8.boundary, applyAliasNames(boxes));
    if (verdict.ok)
      return { html: 'Looks good. Press <span class="btn-ref">Check</span>.' };
    const hasReset = !!document.getElementById("p8-reset");
    return hasReset
      ? {
          html: 'Your program has a problem that isn\'t covered by a hint. Try starting over on this step by clicking <span class="btn-ref">Reset</span>.',
        }
      : "Your program has a problem that isn't covered by a hint. Sorry.";
  }

  function renderCode() {
    const progress = editableSteps.has(p8.boundary) && !p8.passes[p8.boundary];
    renderCodePane($("#p8-code"), p8.lines, p8.boundary, { progress });
  }

  function render() {
    renderCode();
    const stage = $("#p8-stage");
    stage.innerHTML = "";
    if (editableSteps.has(p8.boundary) && p8.passes[p8.boundary]) {
      $("#p8-status").textContent = "correct";
      $("#p8-status").className = "ok";
    } else {
      $("#p8-status").textContent = "";
      $("#p8-status").className = "muted";
    }
    $("#p8-check").classList.add("hidden");
    $("#p8-reset").classList.add("hidden");
    $("#p8-add").classList.add("hidden");
    hint.setButtonHidden(true);
    resetHint();

    if (p8.boundary > 0) {
      const editable =
        editableSteps.has(p8.boundary) && !p8.passes[p8.boundary];
      const defaults = defaultsFor(p8.boundary);
      const wrap = restoreWorkspace(
        p8.ws[p8.boundary],
        defaults,
        "p8workspace",
        {
          editable,
          deletable: editable,
          allowNameAdd: false,
          allowNameToggle: true,
          allowNameEdit: null,
          allowTypeEdit: null,
        },
      );
      stage.appendChild(wrap);
      if (editable) {
        $("#p8-check").classList.remove("hidden");
        $("#p8-add").classList.remove("hidden");
        hint.setButtonHidden(false);
        ensureBaseline(p8.boundary, defaults);
        attachResetWatcher(wrap, p8.boundary);
      } else {
        p8.ws[p8.boundary] = cloneBoxes(defaults);
        p8.passes[p8.boundary] = true;
      }
    }
  }

  function save() {
    if (p8.boundary >= 1 && p8.boundary <= p8.lines.length) {
      const snap = serializeWorkspace("p8workspace");
      p8.ws[p8.boundary] = Array.isArray(snap) ? applyAliasNames(snap) : snap;
    }
  }

  $("#p8-reset").onclick = () => {
    if (p8.boundary >= 1) {
      p8.ws[p8.boundary] = null;
      render();
    }
  };

  $("#p8-add").onclick = () => {
    const ws = document.getElementById("p8workspace");
    if (!ws) return;
    ws.appendChild(makeAnswerBox({}));
    updateResetVisibility(p8.boundary);
  };

  $("#p8-check").onclick = () => {
    resetHint();
    if (!editableSteps.has(p8.boundary)) return;
    const ws = document.getElementById("p8workspace");
    if (!ws) return;
    const boxes = [...ws.querySelectorAll(".vbox")].map((v) => readBoxState(v));
    const normalized = applyAliasNames(boxes);
    const verdict = validateWorkspace(p8.boundary, normalized);
    $("#p8-status").textContent = verdict.ok ? "correct" : "incorrect";
    $("#p8-status").className = verdict.ok ? "ok" : "err";
    MB.flashStatus($("#p8-status"));
    if (verdict.ok) {
      p8.passes[p8.boundary] = true;
      p8.ws[p8.boundary] = normalized;
      ws.querySelectorAll(".vbox").forEach((v) => MB.disableBoxEditing(v));
      MB.removeBoxDeleteButtons(ws);
      updateStatus();
      $("#p8-check").classList.add("hidden");
      $("#p8-reset").classList.add("hidden");
      $("#p8-add").classList.add("hidden");
      hint.hide();
      $("#p8-hint-btn")?.classList.add("hidden");
      MB.pulseNextButton("p8");
      renderCode();
      pager.update();
    }
  };

  function validateWorkspace(boundary, boxes) {
    const expected = canonical(boundary);
    const findBox = (need) => {
      return boxes.find((b) => {
        if (b.name === need.name) return true;
        const names = b.names || [];
        return names.includes(need.name);
      });
    };
    for (const need of expected) {
      const box = findBox(need);
      if (!box)
        return {
          ok: false,
          message: `Missing ${need.name} in the program state.`,
        };
      if (need.type === "int" && box.type !== "int")
        return { ok: false, message: `${need.name} must stay type int.` };
      if (need.type === "int*" && box.type !== "int*")
        return { ok: false, message: `${need.name} must be type int*.` };
      if (need.type === "int**" && box.type !== "int**")
        return { ok: false, message: "bear must be type int**." };
      // Addresses are generated; no need to validate them here.
      if (need.type === "int") {
        const val = box.value || "";
        if (need.value === "empty") {
          if (!isEmptyVal(val))
            return {
              ok: false,
              message: `${need.name} should stay empty here.`,
            };
        } else {
          if (val !== need.value)
            return {
              ok: false,
              message: `${need.name} should be ${need.value}.`,
            };
        }
        if (Array.isArray(need.names)) {
          const names = box.names || [];
          if (!need.names.every((n) => names.includes(n)))
            return {
              ok: false,
              message: `Include other name ${need.names.find((n) => !names.includes(n))}.`,
            };
        }
      } else {
        const normalized = normalizePtrValue(box.value || "");
        if (need.value === "empty" && normalized !== "empty")
          return {
            ok: false,
            message: `${need.name} has not been set yet—leave it empty.`,
          };
        if (need.value !== "empty" && normalized !== need.value) {
          if (need.name === "wolf") {
            const targetAddr = need.value;
            const target = targetAddr === String(p8.aAddr) ? "deer" : "hare";
            return {
              ok: false,
              message: `wolf should point to ${target} at address ${targetAddr}.`,
            };
          }
          if (need.name === "bear") {
            return { ok: false, message: "bear should hold wolf's address." };
          }
        }
      }
    }
    return { ok: true };
  }

  const pager = createStepper({
    prefix: "p8",
    lines: p8.lines,
    nextPage: NEXT_PAGE,
    endLabel: "Next Program",
    getBoundary: () => p8.boundary,
    setBoundary: (val) => {
      p8.boundary = val;
    },
    onBeforeChange: save,
    onAfterChange: () => {
      render();
      updateStatus();
    },
    isStepLocked: (boundary) => {
      if (editableSteps.has(boundary)) return !p8.passes[boundary];
      if (boundary === p8.lines.length) return !p8.passes[p8.lines.length];
      return false;
    },
    getStepBadge: (step) => {
      if (!editableSteps.has(step)) return "";
      return p8.passes[step] ? "check" : "note";
    },
  });

  render();
  updateStatus();
  pager.update();
})(window.MB);
