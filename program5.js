(function (MB) {
  const {
    $,
    randAddr,
    vbox,
    isEmptyVal,
    createSimpleSimulator,
    createHintController,
    createStepper,
    pulseNextButton,
    flashStatus,
  } = MB;

  const instructions = $("#p5-instructions");
  const editor = $("#p5-editor");
  const lineNumbers = $("#p5-line-numbers");
  const errorGutter = $("#p5-error-gutter");
  const measureEl = (() => {
    if (!editor || !editor.parentElement) return null;
    const el = document.createElement("div");
    el.className = "code-textarea-measure";
    el.setAttribute("aria-hidden", "true");
    editor.parentElement.appendChild(el);
    return el;
  })();
  const NEXT_PAGE = "program6.html";

  const p5 = {
    text: editor ? editor.value : "",
    expected: [
      { name: "apple", type: "int", value: "10", address: "<i>(any)</i>" },
      { name: "berry", type: "int", value: "5", address: "<i>(any)</i>" },
    ],
    pass: false,
    allocBase: null,
  };

  const simulator = createSimpleSimulator({
    allowVarAssign: true,
    allowDeclAssign: true,
    allowDeclAssignVar: true,
    requireSourceValue: true,
    allowPointers: true,
  });

  function allocFactory() {
    if (p5.allocBase == null) p5.allocBase = randAddr("int");
    let next = p5.allocBase;
    return () => {
      const addr = next;
      next += 4;
      return String(addr);
    };
  }

  function statesMatch(actual, expected) {
    if (!Array.isArray(actual) || !Array.isArray(expected)) return false;
    if (actual.length !== expected.length) return false;
    const byName = Object.fromEntries(expected.map((b) => [b.name, b]));
    for (const b of actual) {
      const exp = byName[b.name];
      if (!exp) return false;
      if (exp.type !== b.type) return false;
      if (String(exp.value || "") !== String(b.value || "")) return false;
    }
    return true;
  }

  const hint = createHintController({
    button: "#p5-hint-btn",
    panel: "#p5-hint",
    build: buildHint,
  });

  function resetHint() {
    hint.hide();
  }

  function updateInstructions() {
    if (!instructions) return;
    if (p5.pass) {
      instructions.textContent = "Program solved!";
      return;
    }
    instructions.textContent = "Write the code yourself.";
  }

  function getEditorText() {
    return editor ? editor.value : p5.text || "";
  }

  function getRawLines() {
    return getEditorText().split(/\r?\n/);
  }

  function classifyLineStatuses(lines) {
    return simulator.classifyLineStatuses(lines, { alloc: allocFactory() });
  }

  function getLineHeightPx() {
    if (!editor) return 32;
    const style = window.getComputedStyle(editor);
    const lh = parseFloat(style.lineHeight);
    return Number.isFinite(lh) ? lh : 32;
  }

  function autoSizeEditor() {
    if (!editor) return;
    editor.style.height = "auto";
    editor.style.height = `${editor.scrollHeight}px`;
  }

  function measureWrapCounts(lines) {
    if (!editor || !measureEl) return lines.map(() => 1);
    const style = window.getComputedStyle(editor);
    const paddingLeft = parseFloat(style.paddingLeft) || 0;
    const paddingRight = parseFloat(style.paddingRight) || 0;
    const contentWidth = Math.max(
      1,
      editor.clientWidth - paddingLeft - paddingRight,
    );
    measureEl.style.width = `${contentWidth}px`;
    measureEl.style.fontFamily = style.fontFamily;
    measureEl.style.fontSize = style.fontSize;
    measureEl.style.fontWeight = style.fontWeight;
    measureEl.style.letterSpacing = style.letterSpacing;
    measureEl.style.lineHeight = style.lineHeight;
    const lineHeight = getLineHeightPx();
    return lines.map((line) => {
      measureEl.textContent = line === "" ? " " : line;
      const h = measureEl.scrollHeight;
      return Math.max(1, Math.ceil(h / lineHeight - 0.01));
    });
  }

  function updateLineGutters() {
    autoSizeEditor();
    const lines = getRawLines();
    const count = Math.max(lines.length, 1);
    const lineHeight = getLineHeightPx();
    const wraps = measureWrapCounts(lines);
    if (lineNumbers) {
      const frag = document.createDocumentFragment();
      for (let i = 1; i <= count; i++) {
        const num = document.createElement("div");
        num.className = "code-line-number";
        num.style.height = `${(wraps[i - 1] || 1) * lineHeight}px`;
        num.textContent = String(i);
        frag.appendChild(num);
      }
      lineNumbers.innerHTML = "";
      lineNumbers.appendChild(frag);
      if (editor) lineNumbers.style.height = `${editor.clientHeight}px`;
    }
    if (errorGutter) {
      const { invalid, incomplete, errorKinds, info } =
        classifyLineStatuses(lines);
      const frag = document.createDocumentFragment();
      for (let i = 0; i < count; i++) {
        const cell = document.createElement("div");
        cell.className = "code-error-line";
        cell.style.height = `${(wraps[i] || 1) * lineHeight}px`;
        if (invalid.has(i)) {
          cell.classList.add("is-invalid");
          const kind = errorKinds?.get(i) || "compile";
          cell.textContent = kind === "ub" ? "💣" : "🚫";
          cell.title =
            kind === "ub"
              ? "Line causes undefined behavior"
              : "Line does not compile";
          if (info?.has(i)) {
            const infoMsg = info.get(i);
            const title =
              infoMsg && typeof infoMsg === "object" ? infoMsg.text : infoMsg;
            const btn = document.createElement("button");
            btn.type = "button";
            btn.className = "error-info-btn";
            btn.textContent = "i";
            btn.title = title || "";
            cell.appendChild(btn);
          }
        } else if (incomplete.has(i)) {
          cell.classList.add("is-incomplete");
          cell.textContent = "...";
          cell.title = "Line is incomplete";
          if (info?.has(i)) {
            const infoMsg = info.get(i);
            const title =
              infoMsg && typeof infoMsg === "object" ? infoMsg.text : infoMsg;
            const btn = document.createElement("button");
            btn.type = "button";
            btn.className = "error-info-btn";
            btn.textContent = "i";
            btn.title = title || "";
            cell.appendChild(btn);
          }
        } else if (info?.has(i)) {
          const infoMsg = info.get(i);
          const title =
            infoMsg && typeof infoMsg === "object" ? infoMsg.text : infoMsg;
          const btn = document.createElement("button");
          btn.type = "button";
          btn.className = "error-info-btn";
          btn.textContent = "i";
          btn.title = title || "";
          cell.appendChild(btn);
        }
        frag.appendChild(cell);
      }
      errorGutter.innerHTML = "";
      errorGutter.appendChild(frag);
      if (editor) errorGutter.style.height = `${editor.clientHeight}px`;
    }
    if (editor) {
      if (lineNumbers) lineNumbers.scrollTop = editor.scrollTop;
      if (errorGutter) errorGutter.scrollTop = editor.scrollTop;
    }
  }

  function buildHint() {
    const currentState = applyUserProgram();
    const expected = p5.expected;
    const match = statesMatch(currentState, expected);
    if (match)
      return { html: 'Looks good. Press <span class="btn-ref">Check</span>.' };

    const text = getEditorText();
    const missingLine = simulator.findMissingSemicolonLine(text);
    if (missingLine != null)
      return {
        html: `You need a semicolon at the end of line ${missingLine}.`,
      };

    const tokens = simulator.tokenizeProgram(text);
    const parsedStatements = simulator.parseStatements(text);
    const redecl = firstRedeclaration(parsedStatements);
    if (redecl)
      return {
        html: `You declared <code class="tok-name">${redecl}</code> more than once.`,
      };
    const hasTokens = tokens.some(
      (t) => !(t.type === "sym" && t.value === ";"),
    );
    const declNames = new Set(
      parsedStatements
        .filter(
          (s) =>
            s.kind === "decl" ||
            s.kind === "declAssign" ||
            s.kind === "declAssignVar" ||
            s.kind === "declAssignRef",
        )
        .map((s) => s.name),
    );
    if (!hasTokens || !declNames.has("apple"))
      return {
        html: 'Declare apple: try <code class="tok-line">int apple;</code>.',
      };
    if (!declNames.has("berry"))
      return {
        html: 'Declare berry: try <code class="tok-line">int berry;</code>.',
      };
    if (Array.isArray(currentState)) {
      const byName = Object.fromEntries(currentState.map((b) => [b.name, b]));
      for (const exp of expected) {
        const actual = byName[exp.name];
        if (!actual) continue;
        const actualVal = String(actual.value ?? "");
        const expectedVal = String(exp.value ?? "");
        if (!isEmptyVal(actualVal) && actualVal !== expectedVal) {
          return {
            html: `<code class="tok-name">${exp.name}</code>\'s value should be <code class="tok-value">${expectedVal}</code>, not <code class="tok-value">${actualVal}</code>.`,
          };
        }
      }
    }
    const appleAssign = parsedStatements.find(
      (s) =>
        (s.kind === "assign" || s.kind === "declAssign") &&
        s.name === "apple" &&
        String(s.value) === "10",
    );
    if (!appleAssign)
      return {
        html: 'Store 10 in apple with <code class="tok-line">apple = 10;</code>.',
      };
    const berryAssign = parsedStatements.find(
      (s) =>
        (s.kind === "assign" || s.kind === "declAssign") &&
        s.name === "berry" &&
        String(s.value) === "5",
    );
    if (!berryAssign)
      return {
        html: 'Store 5 in berry with <code class="tok-line">berry = 5;</code>.',
      };
    return {
      html: "Keep lines to simple declarations or assignments ending with semicolons.",
    };
  }

  function applyUserProgram() {
    const text = getEditorText();
    p5.text = text;
    return simulator.applyProgram(text, { alloc: allocFactory() });
  }

  function getProgramOutcome() {
    const lines = getRawLines();
    const status = classifyLineStatuses(lines);
    let hasCompile = status.incomplete.size > 0;
    let hasUb = false;
    if (status.errorKinds) {
      for (const kind of status.errorKinds.values()) {
        if (kind === "ub") hasUb = true;
        else hasCompile = true;
      }
    }
    if (hasCompile) return { kind: "compile", state: null };
    if (hasUb) return { kind: "ub", state: null };
    const state = applyUserProgram();
    if (!state) return { kind: "compile", state: null };
    return { kind: "ok", state };
  }

  function renderStage() {
    const stage = $("#p5-stage");
    stage.innerHTML = "";
    const expected = p5.expected;
    const outcome = getProgramOutcome();
    const group = document.createElement("div");
    group.className = "state-group two-col";
    group.appendChild(
      renderState("Your code's final state", outcome.state, outcome.kind),
    );
    group.appendChild(renderState("Target final state", expected));
    stage.appendChild(group);
  }

  function renderState(title, boxes, status = "ok") {
    const wrap = document.createElement("div");
    wrap.className = "state-panel";
    const heading = document.createElement("div");
    heading.className = "state-heading";
    heading.textContent = title;
    wrap.appendChild(heading);
    const grid = document.createElement("div");
    grid.className = "grid";
    if (status === "compile") {
      const msg = document.createElement("div");
      msg.className = "muted state-status";
      msg.style.padding = "8px";
      msg.textContent = "(this code is not valid)";
      grid.appendChild(msg);
    } else if (status === "ub") {
      const msg = document.createElement("div");
      msg.className = "muted state-status";
      msg.style.padding = "8px";
      const label = document.createElement("span");
      label.textContent = "Kaboom! ";
      msg.appendChild(label);
      const link = document.createElement("button");
      link.type = "button";
      link.className = "ub-explain-link";
      link.textContent = "[What is undefined behavior?]";
      const explain = document.createElement("div");
      explain.className = "ub-explain hidden";
      explain.textContent =
        "Undefined behavior means the C standard does not define what happens. The program might crash, act strangely, or appear to work.";
      link.addEventListener("click", (e) => {
        e.preventDefault();
        e.stopPropagation();
        explain.classList.toggle("hidden");
      });
      msg.appendChild(link);
      msg.appendChild(explain);
      grid.appendChild(msg);
    } else if (boxes === null) {
      const msg = document.createElement("div");
      msg.className = "muted state-status";
      msg.style.padding = "8px";
      msg.textContent = "(this code is not valid)";
      grid.appendChild(msg);
    } else if (boxes.length === 0) {
      const msg = document.createElement("div");
      msg.className = "muted";
      msg.style.padding = "8px";
      msg.textContent = "(no variables yet)";
      grid.appendChild(msg);
    } else {
      boxes.forEach((b) => {
        const node = vbox({
          addr: b.address,
          type: b.type,
          value: b.value,
          name: b.name,
          names: b.names,
          editable: false,
          allowNameToggle: true,
        });
        if (isEmptyVal(b.value || ""))
          node.querySelector(".value").classList.add("placeholder", "muted");
        grid.appendChild(node);
      });
    }
    wrap.appendChild(grid);
    return wrap;
  }

  function firstRedeclaration(statements) {
    const seen = new Set();
    for (const stmt of statements) {
      if (
        stmt.kind === "decl" ||
        stmt.kind === "declAssign" ||
        stmt.kind === "declAssignVar" ||
        stmt.kind === "declAssignRef"
      ) {
        if (seen.has(stmt.name)) return stmt.name;
        seen.add(stmt.name);
      }
    }
    return null;
  }

  function render() {
    if (editor && editor.value !== p5.text) editor.value = p5.text;
    updateLineGutters();
    renderStage();
    resetHint();
    if (p5.pass) {
      $("#p5-status").textContent = "correct";
      $("#p5-status").className = "ok";
    } else {
      $("#p5-status").textContent = "";
      $("#p5-status").className = "muted";
    }
    const editable = !p5.pass;
    const checkBtn = $("#p5-check");
    const hintBtn = $("#p5-hint-btn");
    if (editable) {
      checkBtn?.classList.remove("hidden");
      hintBtn?.classList.remove("hidden");
    } else {
      checkBtn?.classList.add("hidden");
      hintBtn?.classList.add("hidden");
    }
    if (editor) editor.readOnly = !editable;
    updateInstructions();
  }

  if (editor) {
    editor.addEventListener("input", () => {
      p5.text = editor.value;
      updateLineGutters();
      renderStage();
    });
    if (lineNumbers) {
      editor.addEventListener("scroll", () => {
        lineNumbers.scrollTop = editor.scrollTop;
      });
    }
    if (errorGutter) {
      editor.addEventListener("scroll", () => {
        errorGutter.scrollTop = editor.scrollTop;
      });
    }
    editor.addEventListener("mouseup", updateLineGutters);
    window.addEventListener("resize", updateLineGutters);
    if (typeof ResizeObserver !== "undefined") {
      const ro = new ResizeObserver(() => updateLineGutters());
      ro.observe(editor);
    }
  }

  $("#p5-check").onclick = () => {
    const state = applyUserProgram();
    const expected = p5.expected;
    const match = statesMatch(state, expected);
    $("#p5-status").textContent = match ? "correct" : "incorrect";
    $("#p5-status").className = match ? "ok" : "err";
    flashStatus($("#p5-status"));
    if (match) {
      const ws = document.getElementById("p5-stage");
      ws?.querySelectorAll(".vbox").forEach((v) => MB.disableBoxEditing(v));
      MB.removeBoxDeleteButtons(ws);
      p5.pass = true;
      if (editor) editor.readOnly = true;
      $("#p5-check").classList.add("hidden");
      hint.hide();
      $("#p5-hint-btn")?.classList.add("hidden");
      pulseNextButton("p5");
      pager.update();
    }
  };

  const pager = createStepper({
    prefix: "p5",
    lines: 0,
    nextPage: NEXT_PAGE,
    getBoundary: () => 0,
    setBoundary: () => {},
    onAfterChange: render,
    isStepLocked: () => !p5.pass,
  });

  render();
  pager.update();
})(window.MB);
