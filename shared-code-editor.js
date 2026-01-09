(function (global) {
  const MB = global.MB || {};
  const {
    createSimpleSimulator,
    randAddr,
    typeInfo,
    vbox,
    renderParts,
    setPartsContent,
    flashStatus,
    createStepper,
    ensureBaseLayout,
    resolveActiveNavItem,
  } = MB;

  function collectCodeEditorElements(root = document) {
    return {
      instructionsEl: root.querySelector('[data-role="code-instructions"]'),
      editor: root.querySelector('[data-role="code-editor"]'),
      lineNumbers: root.querySelector('[data-role="code-line-numbers"]'),
      errorGutter: root.querySelector('[data-role="code-error-gutter"]'),
      stage: root.querySelector('[data-role="code-stage"]'),
      status: root.querySelector('[data-role="code-status"]'),
      hintPanel: root.querySelector('[data-role="code-hint"]'),
      hintBtn: root.querySelector('[data-role="code-hint-btn"]'),
      checkBtn: root.querySelector('[data-role="code-check"]'),
      codeRoot: root.querySelector('[data-role="code-panel"]'),
    };
  }

  function ensureCodeEditorLayout({
    pageTitle,
    browserTitle,
    navItems,
    activeHref,
    textareaMinLines,
  } = {}) {
    const activeItem = resolveActiveNavItem(navItems, activeHref);
    const resolvedTitle = pageTitle || activeItem?.label || "";
    const nextBrowserTitle =
      browserTitle || (resolvedTitle ? `C Boxes - ${resolvedTitle}` : "");
    if (nextBrowserTitle) document.title = nextBrowserTitle;
    const existing = document.querySelector('[data-role="code-editor"]');
    if (existing) return collectCodeEditorElements();

    const { main } = ensureBaseLayout({ navItems, activeHref });
    if (resolvedTitle) {
      const heading = document.createElement("h1");
      heading.className = "page-title";
      heading.textContent = resolvedTitle;
      main.appendChild(heading);
    }

    const instructionsEl = document.createElement("p");
    instructionsEl.dataset.role = "code-instructions";
    instructionsEl.className = "intro";
    main.appendChild(instructionsEl);

    const section = document.createElement("section");
    const row = document.createElement("div");
    row.className = "row";
    section.appendChild(row);
    main.appendChild(section);

    const codePanel = document.createElement("div");
    codePanel.className = "panel code-editor-panel";
    codePanel.dataset.role = "code-panel";
    const codeTitle = document.createElement("div");
    codeTitle.className = "panel-title code-title";
    codeTitle.textContent = "Code";
    const codePane = document.createElement("div");
    codePane.className = "codepane";
    const codeRow = document.createElement("div");
    codeRow.className = "codepane-row";
    const lineNumbers = document.createElement("div");
    lineNumbers.dataset.role = "code-line-numbers";
    lineNumbers.className = "code-gutter";
    lineNumbers.setAttribute("aria-hidden", "true");
    const editorWrap = document.createElement("div");
    editorWrap.className = "code-editor-wrap";
    const editor = document.createElement("textarea");
    editor.dataset.role = "code-editor";
    editor.className = "code-textarea";
    editor.spellcheck = false;
    const rows = Number.isFinite(textareaMinLines)
      ? Math.max(1, Number(textareaMinLines))
      : 4;
    editor.setAttribute("rows", String(rows));
    editorWrap.appendChild(editor);
    const errorGutter = document.createElement("div");
    errorGutter.dataset.role = "code-error-gutter";
    errorGutter.className = "code-error-gutter";
    errorGutter.setAttribute("aria-hidden", "true");
    codeRow.appendChild(lineNumbers);
    codeRow.appendChild(editorWrap);
    codeRow.appendChild(errorGutter);
    codePane.appendChild(codeRow);
    const codeControls = document.createElement("div");
    codeControls.className = "controls";
    const nextBtn = document.createElement("button");
    nextBtn.textContent = "Next Program ▶▶";
    nextBtn.dataset.stepper = "next";
    codeControls.appendChild(nextBtn);
    codePanel.appendChild(codeTitle);
    codePanel.appendChild(codePane);
    codePanel.appendChild(codeControls);

    const statePanel = document.createElement("div");
    statePanel.className = "panel code-editor-panel";
    const stage = document.createElement("div");
    stage.dataset.role = "code-stage";
    const stateControls = document.createElement("div");
    stateControls.className = "controls";
    const checkBtn = document.createElement("button");
    checkBtn.dataset.role = "code-check";
    checkBtn.textContent = "Check";
    const hintBtn = document.createElement("button");
    hintBtn.dataset.role = "code-hint-btn";
    hintBtn.type = "button";
    hintBtn.className = "hint-link";
    hintBtn.textContent = "Hint";
    const status = document.createElement("span");
    status.dataset.role = "code-status";
    status.className = "muted";
    stateControls.appendChild(checkBtn);
    stateControls.appendChild(hintBtn);
    stateControls.appendChild(status);
    const hintPanel = document.createElement("div");
    hintPanel.dataset.role = "code-hint";
    hintPanel.className = "hint-inline hidden";
    statePanel.appendChild(stage);
    statePanel.appendChild(stateControls);
    statePanel.appendChild(hintPanel);

    row.appendChild(codePanel);
    row.appendChild(statePanel);

    return {
      instructionsEl,
      editor,
      lineNumbers,
      errorGutter,
      stage,
      status,
      hintPanel,
      hintBtn,
      checkBtn,
      codeRoot: codePanel,
    };
  }

  function createCodeEditorTemplate(config = {}) {
    const {
      startCode = "",
      targetState = [],
      textareaWidth = null,
      textareaMinLines = null,
      allowNewLines = true,
      hints = null,
      instructions = null,
      next = null,
      pageTitle = null,
      browserTitle = null,
      navItems = null,
      activeHref = null,
    } = config;

    const {
      instructionsEl,
      editor,
      lineNumbers,
      errorGutter,
      stage,
      status,
      hintPanel,
      hintBtn,
      checkBtn,
      codeRoot,
    } = ensureCodeEditorLayout({
      pageTitle,
      browserTitle,
      navItems,
      activeHref,
      textareaMinLines,
    });

    const measureEl = (() => {
      if (!editor || !editor.parentElement) return null;
      const el = document.createElement("div");
      el.className = "code-textarea-measure";
      el.setAttribute("aria-hidden", "true");
      editor.parentElement.appendChild(el);
      return el;
    })();

    const state = {
      text: editor ? editor.value : "",
      pass: false,
      allocBase: null,
    };
    let pager = null;

    function normalizeEditorText(text) {
      if (allowNewLines) return String(text ?? "");
      const normalized = String(text ?? "").replace(/\r\n/g, "\n");
      return normalized.replace(/\n/g, " ");
    }

    function adjustSelectionForCarriageReturns(text, pos) {
      if (!Number.isFinite(pos)) return pos;
      let removed = 0;
      for (let i = 0; i < pos && i < text.length; i++) {
        if (text[i] === "\r") removed += 1;
      }
      return Math.max(0, pos - removed);
    }

    if (startCode != null && String(startCode) !== "") {
      state.text = normalizeEditorText(startCode);
      if (editor) editor.value = state.text;
    }
    if (editor) {
      if (textareaWidth) {
        editor.style.width = String(textareaWidth);
      }
      if (Number.isFinite(textareaMinLines)) {
        const lines = Math.max(1, Number(textareaMinLines));
        editor.style.minHeight = `calc(var(--code-line-height) * ${lines} + 16px)`;
      }
    }

    const simulator = createSimpleSimulator({
      allowVarAssign: true,
      allowDeclAssign: true,
      allowDeclAssignVar: true,
      requireSourceValue: true,
      allowPointers: true,
    });

    function allocFactory() {
      if (state.allocBase == null) state.allocBase = randAddr("int");
      let next = Number(state.allocBase);
      return (type = "int") => {
        const info = typeInfo(type || "int");
        const size = info.size || 4;
        const align = info.align || 1;
        if (next % align !== 0) {
          next = Math.ceil(next / align) * align;
        }
        const addr = next;
        next = addr + size;
        return String(addr);
      };
    }

    function getEditorText() {
      return editor ? editor.value : state.text || "";
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

    function applyUserProgram() {
      const text = getEditorText();
      state.text = text;
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
            address: b.address,
            type: b.type,
            value: b.value,
            name: b.name,
            editable: false,
          });
          if (String(b.value ?? "") === "")
            node.querySelector(".value").classList.add("placeholder", "muted");
          grid.appendChild(node);
        });
      }
      wrap.appendChild(grid);
      return wrap;
    }

    function renderStage() {
      if (!stage) return;
      stage.innerHTML = "";
      const outcome = getProgramOutcome();
      const group = document.createElement("div");
      group.className = "state-group two-col";
      group.appendChild(
        renderState("Your code's final state", outcome.state, outcome.kind),
      );
      group.appendChild(renderState("Target final state", targetState));
      stage.appendChild(group);
    }

    function makeToken(role, text) {
      return { kind: "tok", role, text: String(text ?? "") };
    }

    function partsContext({ state: programState, outcome } = {}) {
      return {
        passed: !!state.pass,
        text: getEditorText(),
        lines: getRawLines(),
        state: programState,
        outcome,
        targetState,
        nameStyle: (text) => makeToken("name", text),
        typeStyle: (text) => makeToken("type", text),
        valueStyle: (text) => makeToken("value", text),
        addrStyle: (text) => makeToken("addr", text),
        codeStyle: (text) => makeToken("code", text),
        btnStyle: (text) => makeToken("btn", text),
        simulator,
        tokenizeProgram: simulator.tokenizeProgram,
        parseStatements: simulator.parseStatements,
        findMissingSemicolonLine: simulator.findMissingSemicolonLine,
        findMissingSemicolonLines: simulator.findMissingSemicolonLines,
        classifyLineStatuses,
        applyUserProgram,
      };
    }

    function setStatus(text, cls = "muted") {
      if (!status) return;
      status.textContent = text;
      status.className = cls;
    }

    function updateInstructions() {
      const outcome = getProgramOutcome();
      const ctx = partsContext({ state: outcome.state, outcome });
      if (state.pass) {
        setPartsContent(instructionsEl, "Program solved!");
        return;
      }
      if (typeof instructions === "function") {
        const parts = instructions(ctx);
        setPartsContent(instructionsEl, parts);
        return;
      }
      if (instructions) {
        setPartsContent(instructionsEl, instructions);
        return;
      }
      setPartsContent(instructionsEl, []);
    }

    function hideHint() {
      if (!hintPanel) return;
      hintPanel.textContent = "";
      hintPanel.classList.add("hidden");
    }

    function showHint(parts) {
      if (!hintPanel) return;
      if (!parts || (Array.isArray(parts) && parts.length === 0)) return;
      renderParts(hintPanel, parts);
      hintPanel.classList.remove("hidden");
      flashStatus(hintPanel);
    }

    function render() {
      renderStage();
      updateInstructions();
      updateLineGutters();
      if (state.pass) {
        setStatus("correct", "ok");
      } else {
        setStatus("", "muted");
      }
      const editable = !state.pass;
      if (checkBtn) checkBtn.classList.toggle("hidden", !editable);
      if (hintBtn) hintBtn.classList.toggle("hidden", !editable);
      if (editor) editor.readOnly = !editable;
      if (!editable) {
        editor?.classList.add("readonly");
      }
    }

    function evaluate() {
      const outcome = getProgramOutcome();
      const ok =
        outcome.kind === "ok" && statesMatch(outcome.state, targetState);
      return { ok, outcome };
    }

    function handleHint() {
      const result = evaluate();
      const ctx = partsContext({
        state: result.outcome.state,
        outcome: result.outcome,
      });
      if (result.ok) {
        showHint("Looks good. Press $b{Check}.");
        return;
      }
      let parts = null;
      if (typeof hints === "function") {
        parts = hints(ctx);
      } else {
        parts = hints;
      }
      if (!parts || (Array.isArray(parts) && parts.length === 0)) {
        showHint(
          "Your program has a problem that isn't covered by a hint, sorry.",
        );
        return;
      }
      showHint(parts);
    }

    if (editor) {
      editor.addEventListener("input", () => {
        const raw = editor.value;
        const next = normalizeEditorText(raw);
        if (next !== raw) {
          const start = adjustSelectionForCarriageReturns(
            raw,
            editor.selectionStart,
          );
          const end = adjustSelectionForCarriageReturns(
            raw,
            editor.selectionEnd,
          );
          editor.value = next;
          if (
            Number.isFinite(start) &&
            Number.isFinite(end) &&
            typeof editor.setSelectionRange === "function"
          ) {
            const clampedStart = Math.min(next.length, start);
            const clampedEnd = Math.min(next.length, end);
            editor.setSelectionRange(clampedStart, clampedEnd);
          }
        }
        state.text = editor.value;
        renderStage();
        updateLineGutters();
      });
      if (!allowNewLines) {
        editor.addEventListener("keydown", (event) => {
          if (event.key === "Enter") event.preventDefault();
        });
      }
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

    if (hintBtn) {
      hintBtn.addEventListener("click", () => {
        hideHint();
        handleHint();
      });
    }

    if (checkBtn) {
      checkBtn.addEventListener("click", () => {
        hideHint();
        const result = evaluate();
        setStatus(
          result.ok ? "correct" : "incorrect",
          result.ok ? "ok" : "err",
        );
        flashStatus(status);
        if (!result.ok) return;
        state.pass = true;
        if (editor) editor.readOnly = true;
        checkBtn?.classList.add("hidden");
        hintBtn?.classList.add("hidden");
        pager?.pulseNext();
        render();
        pager?.update();
      });
    }

    pager = createStepper({
      root: codeRoot || editor?.closest(".panel") || document.body,
      lines: 0,
      nextPage: next || null,
      endLabel: "Next Program",
      getBoundary: () => 0,
      setBoundary: () => {},
      onAfterChange: render,
      isStepLocked: () => !state.pass,
    });

    render();
    pager.update();
  }

  MB.createCodeEditorTemplate = createCodeEditorTemplate;
  global.MB = MB;
})(window);
