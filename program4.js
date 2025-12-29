(function (MB) {
  const {
    $,
    randAddr,
    vbox,
    renderCodePaneEditable,
    readEditableCodeLines,
    createHintController,
    createStepper,
    pulseNextButton,
    isEmptyVal,
    parseSimpleStatement,
    applySimpleStatement,
    flashStatus,
    el,
    stripLineComments,
  } = MB;

  const instructions = $("#p4-instructions");
  const NEXT_PAGE = "program5.html";

  const p4 = {
    lines: [{ text: "int rain;", editable: true, placeholder: "" }],
    userLines: {},
    pass: false,
    expected: [
      { name: "cloud", type: "int", value: "empty", address: "<i>(any)</i>" },
    ],
    allocBase: null,
  };

  const hint = createHintController({
    button: "#p4-hint-btn",
    panel: "#p4-hint",
    build: buildHint,
  });

  function resetHint() {
    hint.hide();
  }

  function updateInstructions() {
    if (!instructions) return;
    if (p4.pass) {
      instructions.textContent = "Program solved!";
      return;
    }
    instructions.innerHTML =
      'Until now, you have been editing the program state to match the code. Now you will be editing the code to match the program state. Edit the line so that "Expected final state" and "Your final state" match, then press <span class="btn-ref">Check</span>.';
  }

  function normalizedLines() {
    const map = readEditableCodeLines($("#p4-code"));
    const lines = p4.lines.map((line, idx) => {
      const val = map[idx];
      return {
        ...line,
        text: (val != null ? val : line.text) || "",
      };
    });
    return lines;
  }

  function buildHint() {
    const actual = applyUserProgram();
    const expected = p4.expected;
    const match =
      Array.isArray(actual) &&
      actual.length === expected.length &&
      actual.every(
        (b, i) =>
          b.name === expected[i].name &&
          b.type === expected[i].type &&
          String(b.value || "") === String(expected[i].value || ""),
      );
    if (match)
      return { html: 'Looks good. Press <span class="btn-ref">Check</span>.' };

    const lines = normalizedLines()
      .map((l) => stripLineComments(l.text || ""))
      .map((res) => (res.text || "").trim())
      .filter((t) => t && t !== ";");
    if (!lines.length)
      return {
        html: 'Edit the line to create an empty variable named <code class="tok-name">cloud</code>, of type <code class="tok-type">int</code>. Look at the earlier programs if you forget how this is done.',
      };
    const almostDecl = lines.find((l) =>
      /^int\s+[A-Za-z_][A-Za-z0-9_]*\s*$/.test(l),
    );
    const almostAssign = lines.find((l) =>
      /^[A-Za-z_][A-Za-z0-9_]*\s*=\s*-?\d+\s*$/.test(l),
    );
    if (almostDecl || almostAssign)
      return { html: "You need a semicolon at the end of the line." };
    if (!lines.some((l) => /int\s+cloud\s*;/.test(l))) {
      const wrongName = lines.find((l) =>
        /^int\s+[A-Za-z_][A-Za-z0-9_]*\s*;/.test(l),
      );
      if (wrongName)
        return {
          html: 'The variable\'s name should be <code class="tok-name">cloud</code>.',
        };
      return {
        html: 'Declare <code class="tok-name">cloud</code> as an <code class="tok-type">int</code>.',
      };
    }
    const assignsTotal = lines.filter((l) => /cloud/.test(l));
    if (assignsTotal.some((l) => /cloud\s*=/.test(l)))
      return {
        html: 'Leave <code class="tok-name">cloud</code> empty—no assignments to <code class="tok-name">cloud</code>.',
      };
    return {
      html: 'Edit the line to create an empty variable named <code class="tok-name">cloud</code>, of type <code class="tok-type">int</code>. Look at the earlier programs if you forget how this is done.',
    };
  }

  function applyUserProgram() {
    const map = readEditableCodeLines($("#p4-code"));
    const lines = p4.lines.map((line, idx) => {
      const text = (map[idx] != null ? map[idx] : line.text) || "";
      return { ...line, text };
    });
    p4.userLines = map;
    let state = [];
    const alloc = allocFactory();
    for (const line of lines) {
      const cleaned = stripLineComments(line.text || "");
      if (cleaned.unterminated) return null;
      const parsed = parseSimpleStatement(cleaned.text);
      const trimmed = (cleaned.text || "").trim();
      if (!parsed) {
        if (!trimmed || trimmed === ";") continue;
        return null;
      }
      const next = applySimpleStatement(state, parsed, { alloc });
      if (!next) return null;
      state = next;
    }
    return state;
  }

  function renderStage() {
    const stage = $("#p4-stage");
    stage.innerHTML = "";
    const expected = p4.expected;
    const actual = applyUserProgram();
    const group = document.createElement("div");
    group.className = "state-group two-col";
    group.appendChild(renderState("Your final state", actual));
    group.appendChild(renderState("Expected final state", expected));
    stage.appendChild(group);
  }

  function renderState(title, boxes) {
    const wrap = document.createElement("div");
    wrap.className = "state-panel";
    const heading = document.createElement("div");
    heading.className = "state-heading";
    heading.textContent = title;
    wrap.appendChild(heading);
    const grid = document.createElement("div");
    grid.className = "grid";
    if (boxes === null) {
      const msg = document.createElement("div");
      msg.className = "muted";
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
          editable: false,
        });
        if (isEmptyVal(b.value || ""))
          node.querySelector(".value").classList.add("placeholder", "muted");
        grid.appendChild(node);
      });
    }
    wrap.appendChild(grid);
    return wrap;
  }

  function linesForRender() {
    return p4.lines.map((line, idx) => ({
      text: (p4.userLines[idx] != null ? p4.userLines[idx] : line.text) || "",
      placeholder: line.placeholder,
      editable: true,
      deletable: p4.lines.length > 1,
    }));
  }

  function invalidLineIndexes() {
    const map = readEditableCodeLines($("#p4-code"));
    const set = new Set();
    let state = [];
    const alloc = allocFactory();
    for (let i = 0; i < p4.lines.length; i++) {
      const text = (map[i] != null ? map[i] : p4.lines[i].text) || "";
      const cleaned = stripLineComments(text);
      if (cleaned.unterminated) {
        set.add(i);
        continue;
      }
      const trimmed = (cleaned.text || "").trim();
      if (!trimmed || trimmed === ";") continue;
      const parsed = parseSimpleStatement(cleaned.text);
      if (!parsed) {
        set.add(i);
        continue;
      }
      const next = applySimpleStatement(state, parsed, { alloc });
      if (!next) {
        set.add(i);
        continue;
      }
      state = next;
    }
    return set;
  }

  function markInvalidLines(set) {
    const code = $("#p4-code .codecol");
    if (!code) return;
    code.querySelectorAll(".line-invalid").forEach((n) => n.remove());
    code.querySelectorAll(".line").forEach((line, idx) => {
      if (!set.has(idx)) return;
      const icon = el(
        '<span class=\"line-invalid\" aria-label=\"Invalid line\" title=\"Line would not compile\">🚫</span>',
      );
      line.appendChild(icon);
    });
  }

  function render() {
    renderCodePaneEditable($("#p4-code"), linesForRender(), null);
    markInvalidLines(invalidLineIndexes());
    renderStage();
    resetHint();
    if (p4.pass) {
      $("#p4-status").textContent = "correct";
      $("#p4-status").className = "ok";
    } else {
      $("#p4-status").textContent = "";
      $("#p4-status").className = "muted";
    }
    const editable = !p4.pass;
    const checkBtn = $("#p4-check");
    const hintBtn = $("#p4-hint-btn");
    if (editable) {
      checkBtn?.classList.remove("hidden");
      hintBtn?.classList.remove("hidden");
    } else {
      checkBtn?.classList.add("hidden");
      hintBtn?.classList.add("hidden");
    }
    updateInstructions();
    bindLineAdd();
  }

  function bindLineAdd() {
    const code = $("#p4-code");
    if (!code) return;
    code.querySelectorAll(".code-editable").forEach((el) => {
      el.oninput = () => {
        renderStage();
        markInvalidLines(invalidLineIndexes());
      };
      el.onkeydown = (e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          addLine();
        }
        if (e.key === "Backspace") {
          const t = MB.txt(el);
          if (t === "") {
            const idx = Number(el.dataset.index || -1);
            if (idx >= 0 && p4.lines.length > 1) {
              removeLine(idx, { focusPrev: true });
              e.preventDefault();
            }
          }
        }
      };
    });
    code.querySelectorAll(".code-delete").forEach((btn) => {
      btn.onclick = () => {
        const idx = Number(btn.dataset.index || -1);
        if (idx >= 0) removeLine(idx);
      };
    });
  }

  function focusLastLine() {
    const code = $("#p4-code");
    if (!code) return;
    const nodes = code.querySelectorAll(".code-editable");
    const last = nodes[nodes.length - 1];
    if (last) {
      last.focus();
      const range = document.createRange();
      range.selectNodeContents(last);
      range.collapse(false);
      const sel = window.getSelection();
      sel.removeAllRanges();
      sel.addRange(range);
    }
  }

  function removeLine(idx, { focusPrev = false } = {}) {
    if (p4.lines.length <= 1) return;
    p4.lines.splice(idx, 1);
    const newMap = {};
    Object.entries(p4.userLines || {}).forEach(([k, v]) => {
      const i = Number(k);
      if (i < idx) newMap[i] = v;
      else if (i > idx) newMap[i - 1] = v;
    });
    p4.userLines = newMap;
    render();
    if (focusPrev) {
      const code = $("#p4-code");
      if (code) {
        const nodes = code.querySelectorAll(".code-editable");
        const target = nodes[Math.max(0, idx - 1)] || nodes[0];
        if (target) {
          target.focus();
          const range = document.createRange();
          range.selectNodeContents(target);
          range.collapse(false);
          const sel = window.getSelection();
          sel.removeAllRanges();
          sel.addRange(range);
        }
      }
    }
  }

  $("#p4-check").onclick = () => {
    const state = applyUserProgram();
    const expected = p4.expected;
    const match =
      Array.isArray(state) &&
      state.length === expected.length &&
      state.every(
        (b, i) =>
          b.name === expected[i].name &&
          b.type === expected[i].type &&
          String(b.value || "") === String(expected[i].value || ""),
      );
    $("#p4-status").textContent = match ? "correct" : "incorrect";
    $("#p4-status").className = match ? "ok" : "err";
    flashStatus($("#p4-status"));
    if (match) {
      const ws = document.getElementById("p4-stage");
      ws?.querySelectorAll(".vbox").forEach((v) => MB.disableBoxEditing(v));
      MB.removeBoxDeleteButtons(ws);
      p4.pass = true;
      // lock code editing once solved
      const code = $("#p4-code");
      code?.querySelectorAll(".code-editable").forEach((el) => {
        el.removeAttribute("contenteditable");
        el.classList.remove("code-editable");
      });
      $("#p4-check").classList.add("hidden");
      hint.hide();
      $("#p4-hint-btn")?.classList.add("hidden");
      pulseNextButton("p4");
      pager.update();
    }
  };

  const pager = createStepper({
    prefix: "p4",
    lines: 0,
    nextPage: NEXT_PAGE,
    getBoundary: () => 0,
    setBoundary: () => {},
    onAfterChange: render,
    isStepLocked: () => !p4.pass,
  });

  render();
  pager.update();
  function allocFactory() {
    if (p4.allocBase == null) p4.allocBase = randAddr("int");
    let next = p4.allocBase;
    return () => {
      const addr = next;
      next += 4;
      return String(addr);
    };
  }
})(window.MB);
