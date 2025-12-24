(function (global) {
  function $(selector, root = document) {
    return root.querySelector(selector);
  }

  function el(html) {
    const t = document.createElement("template");
    t.innerHTML = html.trim();
    return t.content.firstElementChild;
  }

  function typeInfo(type = "int") {
    const clean = (type || "int").trim();
    if (!clean) return { size: 4, align: 4 };
    if (clean.includes("*")) return { size: 8, align: 8 };
    return { size: 4, align: 4 };
  }

  let nextAddr = null;
  function randAddr(type = "int") {
    const { size, align } = typeInfo(type);
    if (nextAddr == null) {
      const base = 64 + Math.floor(Math.random() * 960);
      nextAddr = Math.max(align, Math.ceil(base / align) * align);
    }
    if (nextAddr % align !== 0) {
      nextAddr = Math.ceil(nextAddr / align) * align;
    }
    const addr = nextAddr;
    nextAddr = addr + size;
    return addr;
  }
  randAddr.reset = function (seed = null) {
    nextAddr = seed;
  };

  function isEmptyVal(s) {
    return s.trim() === "" || /^empty$/i.test(s.trim());
  }

  function txt(n) {
    return (n?.textContent || "").trim();
  }

  function renderCodePane(root, lines, boundary, opts = {}) {
    root.innerHTML = "";
    const code = el('<div class="codecol"></div>');
    root.appendChild(code);
    const addBoundary = () =>
      code.appendChild(el('<div class="boundary"></div>'));
    const progress = !!opts.progress;
    const progressIndex = progress && boundary > 0 ? boundary - 1 : -1;
    if (boundary === 0) addBoundary();
    for (let i = 0; i < lines.length; i++) {
      const lr = el('<div class="line"></div>');
      const ln = el(`<div class="ln">${i + 1}</div>`);
      const src = el(`<div class="src">${lines[i]}</div>`);
      if (i < boundary) lr.classList.add("done");
      if (i === progressIndex) lr.classList.add("progress-mid");
      lr.appendChild(ln);
      lr.appendChild(src);
      code.appendChild(lr);
      if (i + 1 === boundary && i !== progressIndex) addBoundary();
    }
  }

  function renderCodePaneEditable(root, lines, boundary = null) {
    root.innerHTML = "";
    const code = el('<div class="codecol"></div>');
    root.appendChild(code);
    const addBoundary = () =>
      code.appendChild(el('<div class="boundary"></div>'));
    if (boundary === 0) addBoundary();
    for (let i = 0; i < lines.length; i++) {
      const line = lines[i] || {};
      const text = line.text ?? "";
      const lr = el('<div class="line code-row"></div>');
      const ln = el(`<div class="ln">${i + 1}</div>`);
      const src = el('<div class="src"></div>');
      src.textContent = text;
      if (line.editable) {
        src.contentEditable = "true";
        src.classList.add("code-editable");
        src.dataset.index = String(i);
        const placeholder = line.placeholder ?? "";
        if (!text.trim()) {
          src.textContent = placeholder;
          if (placeholder) src.classList.add("placeholder", "muted");
        }
        src.dataset.placeholder = placeholder;
      }
      if (boundary != null && i < boundary) lr.classList.add("done");
      lr.appendChild(ln);
      lr.appendChild(src);
      code.appendChild(lr);
      if (boundary != null && i + 1 === boundary) addBoundary();
    }
  }

  function readEditableCodeLines(root) {
    if (!root) return {};
    const map = {};
    root.querySelectorAll(".code-editable").forEach((el) => {
      const idx = Number(el.dataset.index || -1);
      if (idx >= 0) {
        const content = txt(el);
        map[idx] = content;
      }
    });
    return map;
  }

  function parseSimpleStatement(src = "") {
    const raw = (src || "").replace(/\u00a0/g, " ");
    const s = raw.replace(/\s+/g, " ").trim();
    if (!s) return null;
    let m = s.match(/^int\b\s*(\*{0,2})\s*([A-Za-z_][A-Za-z0-9_]*)\s*;$/);
    if (m) {
      const stars = m[1] || "";
      const type = stars === "**" ? "int**" : stars === "*" ? "int*" : "int";
      return { kind: "decl", name: m[2], type };
    }
    m = s.match(/^([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(-?\d+)\s*;$/);
    if (m) return { kind: "assign", name: m[1], value: m[2], valueKind: "num" };
    m = s.match(
      /^([A-Za-z_][A-Za-z0-9_]*)\s*=\s*&\s*([A-Za-z_][A-Za-z0-9_]*)\s*;$/,
    );
    if (m) return { kind: "assignRef", name: m[1], ref: m[2] };
    m = s.match(/^\*\s*([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(-?\d+)\s*;$/);
    if (m) return { kind: "assignDeref", name: m[1], value: m[2] };
    return null;
  }

  function applySimpleStatement(state = [], stmt, opts = {}) {
    const {
      alloc = (type) => String(randAddr(type || "int")),
      allowRedeclare = true,
    } = opts;
    if (!stmt) return cloneBoxes(state);
    const boxes = cloneBoxes(state);
    const by = Object.fromEntries(boxes.map((b) => [b.name, b]));
    if (stmt.kind === "decl") {
      const type = stmt.type || "int";
      if (by[stmt.name] && !allowRedeclare) return null;
      if (!by[stmt.name]) {
        boxes.push({
          name: stmt.name,
          type,
          value: "empty",
          address: alloc(type),
        });
      }
      return boxes;
    }
    if (stmt.kind === "assign" || stmt.kind === "assignRef") {
      const target = by[stmt.name];
      if (!target) return null;
      if (stmt.kind === "assign") {
        if (target.type !== "int") return null;
        target.value = String(stmt.value);
        return boxes;
      }
      // assignRef
      if (target.type !== "int*" && target.type !== "int**") return null;
      const refBox = by[stmt.ref];
      if (!refBox || !refBox.address) return null;
      target.value = String(refBox.address);
      const depth = (target.type.match(/\*/g) || []).length;
      let current = refBox;
      for (let level = 1; level <= depth; level++) {
        const alias = `${"*".repeat(level)}${stmt.name}`;
        const names = current.names || [current.name].filter(Boolean);
        if (!names.includes(alias)) names.push(alias);
        current.names = names;
        if (level === depth) break;
        const nextAddr = current.value;
        if (!nextAddr || nextAddr === "empty") break;
        const nextBox = boxes.find((b) => b.address === String(nextAddr));
        if (!nextBox) break;
        current = nextBox;
      }
      return boxes;
    }
    if (stmt.kind === "assignDeref") {
      const ptr = by[stmt.name];
      if (!ptr || ptr.type !== "int*" || !ptr.value || ptr.value === "empty")
        return null;
      const targetBox = boxes.find((b) => b.address === String(ptr.value));
      if (!targetBox) return null;
      targetBox.value = String(stmt.value);
      const alias = `*${stmt.name}`;
      const names = targetBox.names || [targetBox.name].filter(Boolean);
      if (!names.includes(alias)) names.push(alias);
      targetBox.names = names;
      return boxes;
    }
    return boxes;
  }

  function createSimpleSimulator(opts = {}) {
    const {
      allowVarAssign = false,
      allowDeclAssign = true,
      allowDeclAssignVar = false,
      requireSourceValue = false,
      allowPointers = false,
    } = opts;

    function tokenizeProgram(src = "") {
      const tokens = [];
      let i = 0;
      let line = 0;
      let col = 0;
      while (i < src.length) {
        const ch = src[i];
        if (ch === "\r") {
          i++;
          if (src[i] === "\n") i++;
          line++;
          col = 0;
          continue;
        }
        if (ch === "\n") {
          i++;
          line++;
          col = 0;
          continue;
        }
        if (/\s/.test(ch)) {
          i++;
          col++;
          continue;
        }
        if (ch === "*" || ch === "&" || ch === "=" || ch === ";") {
          tokens.push({ type: "sym", value: ch, line, col });
          i++;
          col++;
          continue;
        }
        if (/[A-Za-z_]/.test(ch)) {
          const startCol = col;
          let j = i + 1;
          while (j < src.length && /[A-Za-z0-9_]/.test(src[j])) j++;
          const ident = src.slice(i, j);
          tokens.push({
            type: ident === "int" ? "kw" : "ident",
            value: ident,
            line,
            col: startCol,
          });
          col += j - i;
          i = j;
          continue;
        }
        if (ch === "-" || /[0-9]/.test(ch)) {
          const startCol = col;
          let j = i;
          if (src[j] === "-") {
            if (!/[0-9]/.test(src[j + 1] || "")) {
              tokens.push({ type: "unknown", value: ch, line, col });
              i++;
              col++;
              continue;
            }
            j++;
          }
          while (j < src.length && /[0-9]/.test(src[j])) j++;
          tokens.push({
            type: "number",
            value: src.slice(i, j),
            line,
            col: startCol,
          });
          col += j - i;
          i = j;
          continue;
        }
        tokens.push({ type: "unknown", value: ch, line, col });
        i++;
        col++;
      }
      return tokens;
    }

    function hasDeclaredPrefix(prefix, names) {
      if (!prefix || !names || !names.size) return false;
      for (const name of names) {
        if (name.startsWith(prefix)) return true;
      }
      return false;
    }

    function resolveDeclType(stars) {
      if (!Number.isFinite(stars) || stars < 0) return null;
      if (stars === 0) return "int";
      if (!allowPointers) return null;
      return `int${"*".repeat(stars)}`;
    }

    function isPointerType(type) {
      const depth = pointerDepth(type);
      return Number.isFinite(depth) && depth > 0;
    }

    function pointerDepth(type) {
      if (type === "int") return 0;
      const match = String(type || "").match(/^int(\*+)$/);
      return match ? match[1].length : null;
    }

    function makePointerType(depth) {
      if (!Number.isFinite(depth) || depth < 0) return null;
      return depth === 0 ? "int" : `int${"*".repeat(depth)}`;
    }

    function isRefCompatible(targetType, refType) {
      const targetDepth = pointerDepth(targetType);
      const refDepth = pointerDepth(refType);
      if (!Number.isFinite(targetDepth) || !Number.isFinite(refDepth))
        return false;
      return targetDepth === refDepth + 1;
    }

    function expectedPointerTypeForRef(refType) {
      const refDepth = pointerDepth(refType);
      if (!Number.isFinite(refDepth)) return null;
      return makePointerType(refDepth + 1);
    }

    function isDeclPrefix(tokens) {
      if (!tokens.length) return false;
      if (tokens[0].type !== "kw" || tokens[0].value !== "int") return false;
      let idx = 1;
      let stars = 0;
      while (
        idx < tokens.length &&
        tokens[idx].type === "sym" &&
        tokens[idx].value === "*"
      ) {
        if (!allowPointers) return false;
        stars++;
        idx++;
      }
      if (!resolveDeclType(stars)) return false;
      if (idx === tokens.length) return true;
      if (tokens[idx].type !== "ident") return false;
      idx++;
      if (idx === tokens.length) return true;
      if (!(allowDeclAssign || allowDeclAssignVar)) return false;
      if (tokens[idx].type !== "sym" || tokens[idx].value !== "=") return false;
      idx++;
      if (idx === tokens.length) return true;
      const rhs = tokens[idx];
      if (rhs.type === "number") {
        return idx === tokens.length - 1 && allowDeclAssign && stars === 0;
      }
      if (rhs.type === "ident") {
        return idx === tokens.length - 1 && allowDeclAssignVar;
      }
      if (rhs.type === "sym" && rhs.value === "&") {
        if (!allowPointers || !allowDeclAssignVar) return false;
        if (!isPointerType(resolveDeclType(stars))) return false;
        if (idx === tokens.length - 1) return true;
        return tokens[idx + 1]?.type === "ident" && idx + 2 === tokens.length;
      }
      return false;
    }

    function isAssignPrefix(tokens, declaredNames) {
      if (!tokens.length) return false;
      if (tokens[0].type !== "ident") return false;
      const name = tokens[0].value;
      if (tokens.length === 1) return hasDeclaredPrefix(name, declaredNames);
      if (tokens[1].type !== "sym" || tokens[1].value !== "=") return false;
      if (!declaredNames?.has(name)) return false;
      if (tokens.length === 2) return true;
      const t2 = tokens[2];
      if (t2.type === "number") {
        return tokens.length === 3;
      }
      if (t2.type === "ident" && allowVarAssign) {
        return (
          tokens.length === 3 && hasDeclaredPrefix(t2.value, declaredNames)
        );
      }
      if (t2.type === "sym" && t2.value === "&" && allowPointers) {
        if (tokens.length === 3) return true;
        return tokens.length === 4 && tokens[3].type === "ident";
      }
      return false;
    }

    function isDerefPrefix(tokens, declaredNames) {
      if (!allowPointers) return false;
      if (!tokens.length) return false;
      let idx = 0;
      while (
        idx < tokens.length &&
        tokens[idx].type === "sym" &&
        tokens[idx].value === "*"
      )
        idx++;
      if (idx === 0) return false;
      if (idx === tokens.length) return true;
      if (tokens[idx].type !== "ident") return false;
      const name = tokens[idx].value;
      if (idx === tokens.length - 1)
        return hasDeclaredPrefix(name, declaredNames);
      idx++;
      if (tokens[idx].type !== "sym" || tokens[idx].value !== "=") return false;
      if (!declaredNames?.has(name)) return false;
      idx++;
      if (idx >= tokens.length) return true;
      const rhs = tokens[idx];
      if (rhs.type === "number") return idx === tokens.length - 1;
      if (rhs.type === "ident" && allowVarAssign) {
        return (
          idx === tokens.length - 1 &&
          hasDeclaredPrefix(rhs.value, declaredNames)
        );
      }
      if (rhs.type === "sym" && rhs.value === "&" && allowPointers) {
        if (idx === tokens.length - 1) return true;
        return idx === tokens.length - 2 && tokens[idx + 1].type === "ident";
      }
      return false;
    }

    function isStatementPrefix(tokens, declaredNames, allowIntPrefix) {
      if (!tokens.length) return false;
      if (tokens.some((t) => t.type === "unknown")) return false;
      if (
        !allowPointers &&
        tokens.some(
          (t) => t.type === "sym" && (t.value === "*" || t.value === "&"),
        )
      )
        return false;
      if (tokens.length === 1) {
        const t0 = tokens[0];
        if (t0.type === "kw" && t0.value === "int") return true;
        if (t0.type === "ident") {
          if (allowIntPrefix && "int".startsWith(t0.value)) return true;
          return hasDeclaredPrefix(t0.value, declaredNames);
        }
        if (allowPointers && t0.type === "sym" && t0.value === "*") return true;
      }
      return (
        isDeclPrefix(tokens) ||
        isAssignPrefix(tokens, declaredNames) ||
        isDerefPrefix(tokens, declaredNames)
      );
    }

    function parseStatementTokens(tokens) {
      if (!tokens.length) return null;
      if (tokens[0].type === "kw" && tokens[0].value === "int") {
        let idx = 1;
        let stars = 0;
        while (
          idx < tokens.length &&
          tokens[idx].type === "sym" &&
          tokens[idx].value === "*"
        ) {
          if (!allowPointers) return null;
          stars++;
          idx++;
        }
        const declType = resolveDeclType(stars);
        if (!declType) return null;
        if (idx >= tokens.length || tokens[idx].type !== "ident") return null;
        const name = tokens[idx].value;
        idx++;
        if (idx === tokens.length) {
          return { kind: "decl", name, type: declType };
        }
        if (!(allowDeclAssign || allowDeclAssignVar)) return null;
        if (tokens[idx].type !== "sym" || tokens[idx].value !== "=")
          return null;
        idx++;
        if (idx >= tokens.length) return null;
        const rhs = tokens[idx];
        if (
          rhs.type === "number" &&
          allowDeclAssign &&
          stars === 0 &&
          idx === tokens.length - 1
        ) {
          return {
            kind: "declAssign",
            name,
            value: rhs.value,
            valueKind: "num",
            declType,
          };
        }
        if (
          rhs.type === "ident" &&
          allowDeclAssignVar &&
          idx === tokens.length - 1
        ) {
          return { kind: "declAssignVar", name, src: rhs.value, declType };
        }
        if (
          allowPointers &&
          allowDeclAssignVar &&
          rhs.type === "sym" &&
          rhs.value === "&" &&
          isPointerType(declType)
        ) {
          const next = tokens[idx + 1];
          if (next?.type === "ident" && idx + 2 === tokens.length) {
            return { kind: "declAssignRef", name, ref: next.value, declType };
          }
        }
        return null;
      }
      if (
        allowPointers &&
        tokens[0].type === "sym" &&
        tokens[0].value === "*"
      ) {
        let idx = 0;
        let depth = 0;
        while (
          idx < tokens.length &&
          tokens[idx].type === "sym" &&
          tokens[idx].value === "*"
        ) {
          depth++;
          idx++;
        }
        if (idx >= tokens.length || tokens[idx].type !== "ident") return null;
        const name = tokens[idx].value;
        idx++;
        if (
          idx >= tokens.length ||
          tokens[idx].type !== "sym" ||
          tokens[idx].value !== "="
        )
          return null;
        idx++;
        if (idx >= tokens.length) return null;
        const rhs = tokens[idx];
        if (rhs.type === "number" && idx === tokens.length - 1) {
          return { kind: "assignDeref", name, value: rhs.value, depth };
        }
        if (
          rhs.type === "ident" &&
          allowVarAssign &&
          idx === tokens.length - 1
        ) {
          return { kind: "assignDerefVar", name, src: rhs.value, depth };
        }
        if (rhs.type === "sym" && rhs.value === "&" && allowPointers) {
          const next = tokens[idx + 1];
          if (next?.type === "ident" && idx + 2 === tokens.length) {
            return { kind: "assignDerefRef", name, ref: next.value, depth };
          }
        }
      }
      if (
        tokens.length === 4 &&
        tokens[0].type === "ident" &&
        tokens[1].type === "sym" &&
        tokens[1].value === "=" &&
        tokens[2].type === "sym" &&
        tokens[2].value === "&" &&
        tokens[3].type === "ident" &&
        allowPointers
      ) {
        return {
          kind: "assignRef",
          name: tokens[0].value,
          ref: tokens[3].value,
        };
      }
      if (
        tokens.length === 3 &&
        tokens[0].type === "ident" &&
        tokens[1].type === "sym" &&
        tokens[1].value === "="
      ) {
        if (tokens[2].type === "number") {
          return {
            kind: "assign",
            name: tokens[0].value,
            value: tokens[2].value,
            valueKind: "num",
          };
        }
        if (tokens[2].type === "ident" && allowVarAssign) {
          return {
            kind: "assignVar",
            name: tokens[0].value,
            src: tokens[2].value,
          };
        }
      }
      return null;
    }

    function parseDeclAssignRefFallback(tokens) {
      if (!allowPointers) return null;
      if (!tokens.length) return null;
      if (tokens[0].type !== "kw" || tokens[0].value !== "int") return null;
      let idx = 1;
      let stars = 0;
      while (
        idx < tokens.length &&
        tokens[idx].type === "sym" &&
        tokens[idx].value === "*"
      ) {
        stars++;
        idx++;
      }
      const declType = resolveDeclType(stars);
      if (!declType) return null;
      if (idx >= tokens.length || tokens[idx].type !== "ident") return null;
      const name = tokens[idx].value;
      idx++;
      if (idx + 2 !== tokens.length - 1) return null;
      if (tokens[idx].type !== "sym" || tokens[idx].value !== "=") return null;
      if (tokens[idx + 1].type !== "sym" || tokens[idx + 1].value !== "&")
        return null;
      if (tokens[idx + 2].type !== "ident") return null;
      return { name, ref: tokens[idx + 2].value, declType };
    }

    function applyAssignVar(state, stmt) {
      const boxes = cloneBoxes(state);
      const by = Object.fromEntries(boxes.map((b) => [b.name, b]));
      const target = by[stmt.name];
      const source = by[stmt.src];
      if (!target || !source) return null;
      if (requireSourceValue && isEmptyVal(source.value ?? "")) return null;
      const sameType = target.type === source.type;
      const isInt = target.type === "int" && source.type === "int";
      const isPtr = sameType && isPointerType(target.type);
      if (!isInt && !isPtr) return null;
      target.value = String(source.value ?? "empty");
      if (isPtr && target.value && target.value !== "empty") {
        const aliasTarget = boxes.find(
          (b) => b.address === String(target.value),
        );
        if (aliasTarget) {
          const alias = `*${stmt.name}`;
          const names = aliasTarget.names || [aliasTarget.name].filter(Boolean);
          if (!names.includes(alias)) names.push(alias);
          aliasTarget.names = names;
        }
      }
      return boxes;
    }

    function applyAssignRef(state, stmt) {
      const boxes = cloneBoxes(state);
      const by = Object.fromEntries(boxes.map((b) => [b.name, b]));
      const target = by[stmt.name];
      const refBox = by[stmt.ref];
      if (!target || !refBox || !refBox.address) return null;
      const targetDepth = pointerDepth(target.type);
      const refDepth = pointerDepth(refBox.type);
      if (!Number.isFinite(targetDepth) || !Number.isFinite(refDepth))
        return null;
      if (targetDepth !== refDepth + 1) return null;
      target.value = String(refBox.address);
      let current = refBox;
      for (let level = 1; level <= targetDepth; level++) {
        const alias = `${"*".repeat(level)}${stmt.name}`;
        const names = current.names || [current.name].filter(Boolean);
        if (!names.includes(alias)) names.push(alias);
        current.names = names;
        if (level === targetDepth) break;
        const nextAddr = current.value;
        if (!nextAddr || nextAddr === "empty") break;
        const nextBox = boxes.find((b) => b.address === String(nextAddr));
        if (!nextBox) break;
        current = nextBox;
      }
      return boxes;
    }

    function resolveDerefTarget(state, ptrName, depth) {
      const by = Object.fromEntries(state.map((b) => [b.name, b]));
      const ptr = by[ptrName];
      if (!ptr) return { error: "missing" };
      let current = ptr;
      for (let i = 0; i < depth; i++) {
        if (!isPointerType(current.type)) return { error: "type" };
        if (!current.value || current.value === "empty")
          return { error: "empty" };
        const next = state.find((b) => b.address === String(current.value));
        if (!next) return { error: "unknown" };
        current = next;
      }
      return { target: current };
    }

    function applyAssignDeref(state, stmt) {
      const boxes = cloneBoxes(state);
      const { target } = resolveDerefTarget(boxes, stmt.name, stmt.depth || 1);
      if (!target) return null;
      const targetDepth = pointerDepth(target.type);
      if (!Number.isFinite(targetDepth)) return null;
      if (stmt.kind === "assignDeref") {
        if (targetDepth !== 0) return null;
        target.value = String(stmt.value);
      } else if (stmt.kind === "assignDerefVar") {
        const by = Object.fromEntries(boxes.map((b) => [b.name, b]));
        const source = by[stmt.src];
        if (!source) return null;
        if (requireSourceValue && isEmptyVal(source.value ?? "")) return null;
        if (pointerDepth(source.type) !== targetDepth) return null;
        target.value = String(source.value ?? "empty");
      } else if (stmt.kind === "assignDerefRef") {
        const by = Object.fromEntries(boxes.map((b) => [b.name, b]));
        const refBox = by[stmt.ref];
        if (!refBox || !refBox.address) return null;
        if (!isRefCompatible(target.type, refBox.type)) return null;
        target.value = String(refBox.address);
      }
      if (targetDepth > 0 && target.value && target.value !== "empty") {
        const aliasTarget = boxes.find(
          (b) => b.address === String(target.value),
        );
        if (aliasTarget) {
          const alias = `${"*".repeat(stmt.depth || 1)}${stmt.name}`;
          const names = aliasTarget.names || [aliasTarget.name].filter(Boolean);
          if (!names.includes(alias)) names.push(alias);
          aliasTarget.names = names;
        }
      }
      return boxes;
    }

    function applyAssignDerefVar(state, stmt) {
      return applyAssignDeref(state, stmt);
    }

    function applyStatement(state, stmt, opts) {
      if (!stmt) return state;
      if (stmt.kind === "declAssign") {
        const decl = {
          kind: "decl",
          name: stmt.name,
          type: stmt.declType || "int",
        };
        const afterDecl = applySimpleStatement(state, decl, opts);
        if (!afterDecl) return null;
        const assign = {
          kind: "assign",
          name: stmt.name,
          value: stmt.value,
          valueKind: "num",
        };
        return applySimpleStatement(afterDecl, assign, opts);
      }
      if (stmt.kind === "declAssignVar") {
        const decl = {
          kind: "decl",
          name: stmt.name,
          type: stmt.declType || "int",
        };
        const afterDecl = applySimpleStatement(state, decl, opts);
        if (!afterDecl) return null;
        const assignVar = { kind: "assignVar", name: stmt.name, src: stmt.src };
        return applyStatement(afterDecl, assignVar, opts);
      }
      if (stmt.kind === "declAssignRef") {
        const decl = {
          kind: "decl",
          name: stmt.name,
          type: stmt.declType || "int*",
        };
        const afterDecl = applySimpleStatement(state, decl, opts);
        if (!afterDecl) return null;
        const assignRef = { kind: "assignRef", name: stmt.name, ref: stmt.ref };
        return applyAssignRef(afterDecl, assignRef);
      }
      if (stmt.kind === "assignVar") {
        return applyAssignVar(state, stmt);
      }
      if (stmt.kind === "assignRef") {
        return applyAssignRef(state, stmt);
      }
      if (
        stmt.kind === "assignDeref" ||
        stmt.kind === "assignDerefVar" ||
        stmt.kind === "assignDerefRef"
      ) {
        return applyAssignDeref(state, stmt);
      }
      return applySimpleStatement(state, stmt, opts);
    }

    function missingDeclError(name, typeLabel = "int") {
      const text = `You can't assign to ${name} before declaring it. You need to first declare it (${typeLabel} ${name};) prior to this line.`;
      const html = `You can't assign to <code class="tok-name">${name}</code> before declaring it. You need to first declare it (<code class="tok-line">${typeLabel} ${name};</code>) prior to this line.`;
      return { error: { text, html }, kind: "compile" };
    }

    function typeMismatchError(name, expectedType) {
      const text = `${name}'s type would need to be ${expectedType} for this line to work.`;
      const html = `<code class="tok-name">${name}</code>'s type would need to be <code class="tok-type">${expectedType}</code> for this line to work.`;
      return { error: { text, html }, kind: "compile" };
    }

    function describeTokensError(tokens, seenDecl) {
      if (!tokens.length) return "Line has an error.";
      if (tokens.some((t) => t.type === "unknown"))
        return "That line has a character that does not belong in a declaration or assignment.";
      if (
        !allowPointers &&
        tokens.some(
          (t) => t.type === "sym" && (t.value === "*" || t.value === "&"),
        )
      ) {
        return "Pointers are not supported here.";
      }
      if (tokens[0].type === "kw" && tokens[0].value === "int") {
        if (tokens.length === 1) return "A declaration needs a variable name.";
        if (
          tokens.length >= 2 &&
          tokens[1].type === "sym" &&
          tokens[1].value === "*"
        ) {
          let idx = 1;
          while (
            idx < tokens.length &&
            tokens[idx].type === "sym" &&
            tokens[idx].value === "*"
          )
            idx++;
          if (tokens[idx]?.type !== "ident")
            return "A declaration needs a variable name.";
          if (
            tokens[idx + 1]?.type === "sym" &&
            tokens[idx + 1].value === "=" &&
            tokens[idx + 2]?.type === "number"
          ) {
            return 'Pointer declarations should assign from an address, like "int* name = &x;".';
          }
          return "A declaration needs a variable name.";
        }
        if (tokens[1].type !== "ident")
          return "A declaration needs a variable name.";
        if (allowDeclAssign || allowDeclAssignVar)
          return 'Declarations should look like "int name;" or "int name = value;".';
        return 'Declarations should look like "int name;".';
      }
      if (
        allowPointers &&
        tokens[0].type === "sym" &&
        tokens[0].value === "*"
      ) {
        return 'Assignments through pointers should look like "*name = value;".';
      }
      if (tokens[0].type === "ident") {
        const name = tokens[0].value;
        if (!hasDeclaredPrefix(name, seenDecl))
          return `You can't use ${name} before declaring it.`;
        if (tokens.length === 1)
          return 'Assignments should look like "name = value;".';
        if (tokens[1].type !== "sym" || tokens[1].value !== "=")
          return 'Assignments should use "=".';
        if (tokens.length === 2)
          return "Assignment needs a value on the right.";
        const rhs = tokens[2];
        if (rhs.type === "ident" && !allowVarAssign)
          return "Assignments should use a number.";
        if (rhs.type === "ident" && !hasDeclaredPrefix(rhs.value, seenDecl)) {
          return `You can't use ${rhs.value} before declaring it.`;
        }
        return 'Assignments should look like "name = value;".';
      }
      return "Line should be a declaration or assignment.";
    }

    function validateStatement(tokens, state, seenDecl, alloc, lineNumber) {
      if (tokens.some((t) => t.type === "unknown")) {
        return {
          error:
            "That line has a character that does not belong in a declaration or assignment.",
          kind: "compile",
        };
      }
      if (
        !allowPointers &&
        tokens.some(
          (t) => t.type === "sym" && (t.value === "*" || t.value === "&"),
        )
      ) {
        return { error: "Pointers are not supported here.", kind: "compile" };
      }
      const parsed = parseStatementTokens(tokens);
      if (!parsed) {
        const refAssign = parseDeclAssignRefFallback(tokens);
        if (refAssign) {
          const by = Object.fromEntries(state.map((b) => [b.name, b]));
          const refBox = by[refAssign.ref];
          if (!refBox) {
            return {
              error: `You can't use ${refAssign.ref} before declaring it.`,
              kind: "compile",
            };
          }
          const expected = expectedPointerTypeForRef(refBox.type);
          if (expected && refAssign.declType !== expected) {
            return typeMismatchError(refAssign.name, expected);
          }
        }
        return {
          error: describeTokensError(tokens, seenDecl),
          kind: "compile",
        };
      }
      if (
        parsed.kind === "decl" ||
        parsed.kind === "declAssign" ||
        parsed.kind === "declAssignVar" ||
        parsed.kind === "declAssignRef"
      ) {
        if (seenDecl.has(parsed.name))
          return {
            error: `You already declared ${parsed.name}.`,
            kind: "compile",
          };
        if (parsed.kind === "declAssignVar") {
          const by = Object.fromEntries(state.map((b) => [b.name, b]));
          if (!by[parsed.src]) {
            return {
              error: `You can't use ${parsed.src} before declaring it.`,
              kind: "compile",
            };
          }
          if (requireSourceValue && isEmptyVal(by[parsed.src].value ?? "")) {
            return {
              error: `${parsed.src} doesn't have a value yet.`,
              kind: "ub",
            };
          }
        }
        if (parsed.kind === "declAssignRef") {
          const by = Object.fromEntries(state.map((b) => [b.name, b]));
          const refBox = by[parsed.ref];
          if (!refBox) {
            return {
              error: `You can't use ${parsed.ref} before declaring it.`,
              kind: "compile",
            };
          }
          if (!isRefCompatible(parsed.declType, refBox.type)) {
            const expected = expectedPointerTypeForRef(refBox.type);
            if (expected) return typeMismatchError(parsed.name, expected);
            return {
              error: "That assignment is not valid here.",
              kind: "compile",
            };
          }
        }
      } else if (parsed.kind === "assign") {
        if (!seenDecl.has(parsed.name)) {
          return missingDeclError(parsed.name, "int");
        }
      } else if (parsed.kind === "assignVar") {
        const by = Object.fromEntries(state.map((b) => [b.name, b]));
        if (!by[parsed.name]) {
          const typeLabel = by[parsed.src]?.type || "int";
          return missingDeclError(parsed.name, typeLabel);
        }
        if (!by[parsed.src]) {
          return {
            error: `You can't use ${parsed.src} before declaring it.`,
            kind: "compile",
          };
        }
        if (requireSourceValue && isEmptyVal(by[parsed.src].value ?? "")) {
          return {
            error: `${parsed.src} doesn't have a value yet.`,
            kind: "ub",
          };
        }
      } else if (parsed.kind === "assignRef") {
        const by = Object.fromEntries(state.map((b) => [b.name, b]));
        if (!by[parsed.name]) {
          const refType = by[parsed.ref]?.type || "int";
          const typeLabel = expectedPointerTypeForRef(refType) || "int*";
          return missingDeclError(parsed.name, typeLabel);
        }
        const refBox = by[parsed.ref];
        if (!refBox) {
          return {
            error: `You can't use ${parsed.ref} before declaring it.`,
            kind: "compile",
          };
        }
        if (!isPointerType(by[parsed.name].type)) {
          const expected = expectedPointerTypeForRef(refBox.type) || "int*";
          return typeMismatchError(parsed.name, expected);
        }
        if (!isRefCompatible(by[parsed.name].type, refBox.type)) {
          const expected = expectedPointerTypeForRef(refBox.type);
          if (expected) return typeMismatchError(parsed.name, expected);
          return {
            error: "That assignment is not valid here.",
            kind: "compile",
          };
        }
      } else if (
        parsed.kind === "assignDeref" ||
        parsed.kind === "assignDerefVar" ||
        parsed.kind === "assignDerefRef"
      ) {
        const by = Object.fromEntries(state.map((b) => [b.name, b]));
        const ptr = by[parsed.name];
        const depth = parsed.depth || 1;
        if (!ptr) {
          return missingDeclError(
            parsed.name,
            makePointerType(depth) || `int${"*".repeat(depth)}`,
          );
        }
        const ptrDepth = pointerDepth(ptr.type);
        if (!Number.isFinite(ptrDepth) || ptrDepth < depth) {
          return typeMismatchError(
            parsed.name,
            makePointerType(depth) || `int${"*".repeat(depth)}`,
          );
        }
        let expectedPtrDepth = depth;
        if (parsed.kind === "assignDerefVar") {
          if (!by[parsed.src]) {
            return {
              error: `You can't use ${parsed.src} before declaring it.`,
              kind: "compile",
            };
          }
          const srcDepth = pointerDepth(by[parsed.src].type);
          expectedPtrDepth = Number.isFinite(srcDepth)
            ? depth + srcDepth
            : depth;
        }
        if (parsed.kind === "assignDerefRef") {
          if (!by[parsed.ref]) {
            return {
              error: `You can't use ${parsed.ref} before declaring it.`,
              kind: "compile",
            };
          }
          const refDepth = pointerDepth(by[parsed.ref].type);
          expectedPtrDepth = Number.isFinite(refDepth)
            ? depth + refDepth + 1
            : depth + 1;
        }
        if (ptrDepth !== expectedPtrDepth) {
          return typeMismatchError(
            parsed.name,
            makePointerType(expectedPtrDepth) ||
              `int${"*".repeat(expectedPtrDepth)}`,
          );
        }
        let current = ptr;
        for (let i = 0; i < depth; i++) {
          if (!isPointerType(current.type)) {
            return typeMismatchError(
              parsed.name,
              makePointerType(depth) || `int${"*".repeat(depth)}`,
            );
          }
          if (!current.value || current.value === "empty") {
            return {
              error: `${parsed.name} doesn't have a value yet.`,
              kind: "ub",
            };
          }
          const next = state.find((b) => b.address === String(current.value));
          if (!next) {
            return {
              error: `${parsed.name} doesn't point to a known variable.`,
              kind: "ub",
            };
          }
          current = next;
        }
        if (parsed.kind === "assignDerefVar") {
          if (requireSourceValue && isEmptyVal(by[parsed.src].value ?? "")) {
            return {
              error: `${parsed.src} doesn't have a value yet.`,
              kind: "ub",
            };
          }
        }
      }
      const next = applyStatement(state, parsed, {
        alloc,
        allowRedeclare: false,
      });
      if (!next)
        return { error: "That assignment is not valid here.", kind: "compile" };
      return { next, parsed };
    }

    function splitStatements(tokens) {
      const parts = [];
      let current = [];
      let startLine = 0;
      for (const tok of tokens) {
        if (tok.type === "sym" && tok.value === ";") {
          parts.push({
            tokens: current,
            startLine: current[0]?.line ?? startLine,
            endLine: tok.line,
            hasSemicolon: true,
          });
          current = [];
          startLine = tok.line;
          continue;
        }
        if (!current.length) startLine = tok.line;
        current.push(tok);
      }
      if (current.length) {
        parts.push({
          tokens: current,
          startLine: current[0]?.line ?? startLine,
          endLine: current[current.length - 1].line,
          hasSemicolon: false,
        });
      }
      return parts;
    }

    function findMissingSemicolonInTokens(tokens) {
      for (let i = 1; i <= tokens.length; i++) {
        if (parseStatementTokens(tokens.slice(0, i))) {
          if (i < tokens.length) return tokens[i - 1].line;
        }
      }
      return null;
    }

    function findMissingSemicolonLine(text) {
      const tokens = tokenizeProgram(text);
      const parts = splitStatements(tokens);
      for (const part of parts) {
        if (!part.tokens.length) continue;
        const missingInside = findMissingSemicolonInTokens(part.tokens);
        if (missingInside != null) return missingInside + 1;
        const parsed = parseStatementTokens(part.tokens);
        if (parsed && !part.hasSemicolon) return part.endLine + 1;
      }
      return null;
    }

    function parseStatements(text) {
      const tokens = tokenizeProgram(text);
      const parts = splitStatements(tokens);
      const statements = [];
      for (const part of parts) {
        if (!part.tokens.length) continue;
        const parsed = parseStatementTokens(part.tokens);
        if (parsed && part.hasSemicolon) statements.push(parsed);
      }
      return statements;
    }

    function classifyLineStatuses(lines, opts = {}) {
      const invalid = new Set();
      const incomplete = new Set();
      const errors = new Map();
      const errorKinds = new Map();
      const info = new Map();
      const text = lines.join("\n");
      const tokens = tokenizeProgram(text);
      let tokenIndex = 0;
      let currentTokens = [];
      let state = [];
      const alloc = opts.alloc || ((type) => String(randAddr(type || "int")));
      const seenDecl = new Set();
      const escapeHtml = (value) =>
        String(value)
          .replace(/&/g, "&amp;")
          .replace(/</g, "&lt;")
          .replace(/>/g, "&gt;")
          .replace(/"/g, "&quot;")
          .replace(/'/g, "&#39;");
      const toStatementSnippet = (startLine, startCol, endLine) => {
        const safeStart = Math.max(0, startLine || 0);
        const safeEnd = Math.max(safeStart, endLine || 0);
        const parts = [];
        if (safeStart === safeEnd) {
          parts.push((lines[safeStart] || "").slice(startCol || 0));
        } else {
          parts.push((lines[safeStart] || "").slice(startCol || 0));
          for (let i = safeStart + 1; i <= safeEnd; i++) {
            parts.push(lines[i] || "");
          }
        }
        return parts.join("\n").replace(/\n/g, " ").trim();
      };
      for (let lineIndex = 0; lineIndex < lines.length; lineIndex++) {
        let status = "";
        while (
          tokenIndex < tokens.length &&
          tokens[tokenIndex].line === lineIndex
        ) {
          const tok = tokens[tokenIndex];
          if (tok.type === "sym" && tok.value === ";") {
            if (currentTokens.length) {
              const result = validateStatement(
                currentTokens,
                state,
                seenDecl,
                alloc,
                tok.line + 1,
              );
              if (result.error) {
                status = "invalid";
                errors.set(tok.line, result.error);
                errorKinds.set(tok.line, result.kind || "compile");
                const startLine = currentTokens[0]?.line;
                const startCol = currentTokens[0]?.col;
                const errKind = result.kind || "compile";
                if (
                  errKind === "compile" &&
                  Number.isFinite(startLine) &&
                  tok.line > startLine
                ) {
                  const snippet = toStatementSnippet(
                    startLine,
                    startCol,
                    tok.line,
                  );
                  const text = `This statement spans multiple lines and has a compilation error. In C, a line break acts like a space, so your statement is ${snippet}.`;
                  const html = `This statement spans multiple lines and has a compilation error. In C, a line break acts like a space, so your statement is <code class="tok-line">${escapeHtml(snippet)}</code>.`;
                  info.set(tok.line, { text, html });
                }
              } else {
                const startLine = currentTokens[0]?.line;
                const startCol = currentTokens[0]?.col;
                if (Number.isFinite(startLine) && tok.line > startLine) {
                  const text =
                    "This statement spans multiple lines. In C, a line break acts like a space, so it still compiles.";
                  info.set(tok.line, { text, html: text });
                }
                if (
                  result.parsed?.kind === "decl" ||
                  result.parsed?.kind === "declAssign" ||
                  result.parsed?.kind === "declAssignVar"
                ) {
                  seenDecl.add(result.parsed.name);
                }
                state = result.next;
              }
              currentTokens = [];
            }
            tokenIndex++;
            continue;
          }
          currentTokens.push(tok);
          tokenIndex++;
        }
        if (status !== "invalid" && currentTokens.length) {
          const lastLine = currentTokens[currentTokens.length - 1]?.line;
          if (lastLine === lineIndex) {
            const lineText = lines[lineIndex] || "";
            const allowIntPrefix = !/\s$/.test(lineText);
            const isPrefix = isStatementPrefix(
              currentTokens,
              seenDecl,
              allowIntPrefix,
            );
            if (isPrefix) {
              const hasMoreTokens = tokenIndex < tokens.length;
              if (!hasMoreTokens) {
                status = "incomplete";
                const startLine = currentTokens[0]?.line;
                const startCol = currentTokens[0]?.col;
                if (Number.isFinite(startLine) && lineIndex > startLine) {
                  const snippet = toStatementSnippet(
                    startLine,
                    startCol,
                    lineIndex,
                  );
                  const text = `This statement spans multiple lines and is incomplete. In C, a line break acts like a space, so your statement so far is ${snippet}.`;
                  const html = `This statement spans multiple lines and is incomplete. In C, a line break acts like a space, so your statement so far is <code class="tok-line">${escapeHtml(snippet)}</code>.`;
                  info.set(lineIndex, { text, html });
                }
              }
            } else {
              status = "invalid";
              errors.set(
                lineIndex,
                describeTokensError(currentTokens, seenDecl),
              );
              errorKinds.set(lineIndex, "compile");
            }
          }
        }
        if (status === "invalid") invalid.add(lineIndex);
        else if (status === "incomplete") incomplete.add(lineIndex);
      }
      return { invalid, incomplete, errors, errorKinds, info };
    }

    function applyProgram(text, opts = {}) {
      const tokens = tokenizeProgram(text);
      const parts = splitStatements(tokens);
      let state = [];
      const alloc = opts.alloc || ((type) => String(randAddr(type || "int")));
      const seenDecl = new Set();
      for (const part of parts) {
        if (!part.tokens.length) continue;
        const parsed = parseStatementTokens(part.tokens);
        if (!parsed) return null;
        if (!part.hasSemicolon) return null;
        if (
          parsed.kind === "decl" ||
          parsed.kind === "declAssign" ||
          parsed.kind === "declAssignVar"
        ) {
          if (seenDecl.has(parsed.name)) return null;
          seenDecl.add(parsed.name);
        }
        const next = applyStatement(state, parsed, {
          alloc,
          allowRedeclare: false,
        });
        if (!next) return null;
        state = next;
      }
      return state;
    }

    return {
      tokenizeProgram,
      splitStatements,
      parseStatementTokens,
      parseStatements,
      classifyLineStatuses,
      findMissingSemicolonLine,
      applyProgram,
    };
  }

  let nameStackResizeInstalled = false;
  function updateNameStackSpacing(node) {
    if (!node) return;
    const stack = node.querySelector(".name-stack");
    if (!stack) return;
    if (!node.isConnected) return;
    const boxRect = node.getBoundingClientRect();
    const stackRect = stack.getBoundingClientRect();
    const overflow = Math.max(0, Math.ceil(stackRect.bottom - boxRect.bottom));
    node.style.setProperty("--name-stack-space", `${overflow}px`);
  }

  function watchNameStack(node) {
    const stack = node?.querySelector(".name-stack");
    if (!stack) return;
    const update = () => updateNameStackSpacing(node);
    requestAnimationFrame(update);
    if (typeof ResizeObserver !== "undefined") {
      const ro = new ResizeObserver(update);
      ro.observe(stack);
    } else if (!nameStackResizeInstalled) {
      nameStackResizeInstalled = true;
      window.addEventListener("resize", () => {
        document
          .querySelectorAll(".vbox")
          .forEach((box) => updateNameStackSpacing(box));
      });
    }
  }

  function vbox({
    addr = "—",
    type = "int",
    value = "empty",
    name = "",
    names = null,
    editable = false,
    allowNameAdd = false,
    allowNameDelete = false,
    allowNameEdit = false,
    allowTypeEdit = false,
    allowNameToggle = false,
  } = {}) {
    const emptyDisplay = isEmptyVal(String(value || ""));
    const displayValue = emptyDisplay ? "" : value;
    const resolvedNames =
      Array.isArray(names) && names.length
        ? names
        : Array.isArray(name)
          ? name
          : [name];
    const namesList = resolvedNames
      .filter((n) => n !== undefined && n !== null)
      .map((n) => String(n));
    const canToggleNames = allowNameToggle && namesList.length > 1;
    const valueClasses = `value ${editable ? "editable" : ""} ${emptyDisplay ? "placeholder muted" : ""}`;
    const typeClasses = `type ${allowTypeEdit ? "editable" : ""}`;
    const nameClasses = `name-tag ${editable ? "editable" : ""}`;
    const listClasses = `name-list${canToggleNames ? " collapsible" : ""}`;
    const toggleBtn = canToggleNames
      ? '<button class="name-toggle" type="button" aria-expanded="false">Other names</button>'
      : "";
    const addBtn = allowNameAdd
      ? '<button class="name-add" type="button" title="Add name">+</button>'
      : "";
    const nameTags = namesList
      .map((n, idx) => {
        const extraClass = canToggleNames && idx > 0 ? " name-extra" : "";
        const cls =
          namesList.length > 1
            ? `${nameClasses}${extraClass}`
            : `${nameClasses} single`;
        const del = allowNameDelete
          ? `<button class="name-del" data-index="${idx}" type="button" title="Delete name">×</button>`
          : "";
        return `<span class="${cls}"><span class="name-text">${n}</span>${del}</span>`;
      })
      .join("");
    const namesHtml = `${nameTags}${addBtn}${toggleBtn}`;

    const node = el(`
      <div class="vbox ${editable ? "is-editable" : ""}">
        <div class="lbl lbl-addr">address</div>
        <div class="addr">${addr}</div>
          <div class="cell">
          <div class="lbl lbl-value">value</div>
          <div class="${valueClasses}">${displayValue}</div>
        </div>
        <div class="lbl lbl-type">type</div>
        <div class="${typeClasses}">${type}</div>
        <div class="name-stack">
          <div class="${listClasses}">
            <div class="name-list-inner">${namesHtml}</div>
          </div>
          <div class="lbl lbl-name">${namesList.length > 1 ? "name(s)" : "name"}</div>
        </div>
      </div>
    `);

    const scheduleNameStack = () =>
      requestAnimationFrame(() => updateNameStackSpacing(node));

    if (editable) {
      node.querySelector(".value").setAttribute("contenteditable", "true");
      if (allowTypeEdit) {
        const typeEl = node.querySelector(".type");
        typeEl.setAttribute("contenteditable", "true");
        typeEl.classList.add("editable");
      }
      // Names remain read-only for existing boxes; add only for new via allowNameAdd.
      const addBtn = node.querySelector(".name-add");
      if (addBtn) {
        addBtn.onclick = () => {
          const extraClass = canToggleNames ? " name-extra" : "";
          const span = el(
            `<span class="${nameClasses}${extraClass}"><span class="name-text"></span>${allowNameDelete ? '<button class="name-del" type="button" title="Delete name">×</button>' : ""}</span>`,
          );
          addBtn.before(span);
          const textEl = span.querySelector(".name-text");
          if (textEl) {
            if (allowNameEdit) {
              textEl.setAttribute("contenteditable", "true");
              textEl.classList.add("editable");
            }
            textEl.focus();
          }
          const delBtn = span.querySelector(".name-del");
          if (delBtn) {
            delBtn.onclick = () => {
              span.remove();
              scheduleNameStack();
            };
          }
          scheduleNameStack();
        };
      }
      node.querySelectorAll(".name-text").forEach((el) => {
        if (allowNameEdit) {
          el.setAttribute("contenteditable", "true");
          el.classList.add("editable");
        }
      });
    }
    if (canToggleNames) {
      const list = node.querySelector(".name-list");
      const inner = node.querySelector(".name-list-inner");
      const toggle = node.querySelector(".name-toggle");
      if (list && toggle && inner) {
        const clampNames = () => {
          inner.style.transform = "";
          if (!list.classList.contains("expanded")) return;
          if (!list.isConnected) return;
          const listRect = list.getBoundingClientRect();
          const innerRect = inner.getBoundingClientRect();
          if (innerRect.left < listRect.left) {
            const shift = listRect.left - innerRect.left;
            inner.style.transform = `translateX(${shift}px)`;
          }
        };
        const setExpanded = (expanded) => {
          list.classList.toggle("expanded", expanded);
          toggle.setAttribute("aria-expanded", expanded ? "true" : "false");
          toggle.textContent = expanded ? "Hide other names" : "Other names";
          requestAnimationFrame(clampNames);
          scheduleNameStack();
        };
        toggle.onclick = () =>
          setExpanded(!list.classList.contains("expanded"));
        setExpanded(false);
      }
    }
    watchNameStack(node);
    return node;
  }

  function disableBoxEditing(root) {
    if (!root) return;
    root
      .querySelectorAll(".value.editable, .type.editable, .name-text.editable")
      .forEach((el) => {
        el.removeAttribute("contenteditable");
        el.classList.remove("editable");
      });
    root
      .querySelectorAll(".name-add, .name-del")
      .forEach((btn) => btn.remove());
    root.classList.remove("is-editable");
  }

  function removeBoxDeleteButtons(root) {
    const scope = root || document;
    scope.querySelectorAll(".vbox .delete").forEach((btn) => btn.remove());
  }

  function readBoxState(root) {
    if (!root) return null;
    const names = [...root.querySelectorAll(".name-text")]
      .map((el) => txt(el))
      .filter(Boolean);
    const valEl = root.querySelector(".value");
    const valText = txt(valEl);
    const value =
      valEl?.classList?.contains("placeholder") && valText === ""
        ? "empty"
        : valText;
    return {
      addr: txt(root.querySelector(".addr")),
      type: txt(root.querySelector(".type")),
      value,
      name: names[0] || "",
      names,
      nameEditable: !!root.querySelector(".name-text[contenteditable]"),
      typeEditable: !!root.querySelector(".type[contenteditable]"),
    };
  }

  const addrPool = { free: [] };
  function nextPooledAddr(type = "int") {
    if (addrPool.free.length) return addrPool.free.pop();
    return randAddr(type);
  }

  function makeAnswerBox({
    name = "",
    names = null,
    type = "",
    value = "empty",
    address = null,
    editable = true,
    deletable = editable,
    allowNameAdd = false,
    allowNameToggle = false,
    allowNameEdit = null,
    allowTypeEdit = null,
    nameEditable = null,
    typeEditable = null,
  } = {}) {
    const resolvedAddr =
      address == null ? String(nextPooledAddr(type || "int")) : String(address);
    const resolvedNameEdit =
      allowNameEdit !== null && allowNameEdit !== undefined
        ? allowNameEdit
        : nameEditable !== null && nameEditable !== undefined
          ? nameEditable
          : !name && !(Array.isArray(names) && names.length);
    const resolvedTypeEdit =
      allowTypeEdit !== null && allowTypeEdit !== undefined
        ? allowTypeEdit
        : typeEditable !== null && typeEditable !== undefined
          ? typeEditable
          : !type;
    const node = vbox({
      addr: resolvedAddr,
      type,
      value,
      name,
      names,
      editable,
      allowNameAdd,
      allowNameToggle,
      allowNameDelete: allowNameAdd,
      allowNameEdit: resolvedNameEdit,
      allowTypeEdit: resolvedTypeEdit,
    });
    if (deletable) {
      const del = el('<button class="delete" title="delete">×</button>');
      node.appendChild(del);
      del.onclick = () => {
        const addrTxt = txt(node.querySelector(".addr"));
        if (addrTxt) addrPool.free.push(addrTxt);
        node.remove();
      };
    }
    return node;
  }

  function cloneStateBoxes(state) {
    if (!Array.isArray(state)) return [];
    return state
      .filter(Boolean)
      .map((st) => {
        const addr = st.addr ?? st.address ?? null;
        return {
          name: st.name || "",
          names: Array.isArray(st.names)
            ? [...st.names]
            : st.names
              ? [st.names]
              : [],
          type: st.type || "",
          value: st.value ?? "",
          address: addr == null ? null : String(addr),
          nameEditable: st.nameEditable ?? st.allowNameEdit ?? null,
          typeEditable: st.typeEditable ?? st.allowTypeEdit ?? null,
        };
      })
      .filter((st) => st.name);
  }

  function ensureBox(list, spec) {
    const item = list.find((b) => b.name === spec.name);
    if (!item) {
      list.push({
        name: spec.name,
        type: spec.type ?? "",
        value: spec.value ?? "",
        address: spec.address == null ? null : String(spec.address),
      });
      return;
    }
    if (spec.address != null) item.address = String(spec.address);
    if (spec.type && !item.type) item.type = spec.type;
    if (spec.value != null && (item.value == null || item.value === ""))
      item.value = spec.value;
  }

  function cloneBoxes(list) {
    return Array.isArray(list)
      ? list.map((b) => ({
          ...b,
          names: Array.isArray(b.names)
            ? [...b.names]
            : b.names
              ? [b.names]
              : b.name
                ? [b.name]
                : [],
        }))
      : [];
  }

  function firstNonEmptyClone(...states) {
    for (const st of states) {
      const clone = cloneStateBoxes(st);
      if (clone.length) return clone;
    }
    return [];
  }

  function serializeWorkspace(id) {
    const ws = document.getElementById(id);
    if (!ws) return null;
    return [...ws.querySelectorAll(".vbox")].map((v) => readBoxState(v));
  }

  function restoreWorkspace(state, defaults, workspaceId, opts = {}) {
    const {
      editable = true,
      deletable = editable,
      allowNameAdd = false,
      allowNameToggle = false,
      allowNameEdit = null,
      allowTypeEdit = null,
    } = opts;
    const wrap = el(`<div class="grid" id="${workspaceId}"></div>`);
    if (Array.isArray(state) && state.length) {
      state.forEach((st) => {
        const node = makeAnswerBox({
          name: st.name,
          names: st.names,
          type: st.type,
          value: st.value,
          address: st.addr ?? st.address ?? null,
          editable,
          deletable,
          allowNameAdd,
          allowNameToggle,
          allowNameEdit: allowNameEdit ?? st.nameEditable ?? st.allowNameEdit,
          allowTypeEdit: allowTypeEdit ?? st.typeEditable ?? st.allowTypeEdit,
        });
        if (isEmptyVal(st.value))
          node.querySelector(".value").classList.add("placeholder", "muted");
        wrap.appendChild(node);
      });
    } else if (Array.isArray(defaults)) {
      defaults.forEach((d) => {
        const node = makeAnswerBox({
          name: d.name,
          names: d.names,
          type: d.type,
          value: d.value,
          address: d.address ?? d.addr ?? null,
          editable,
          deletable,
          allowNameAdd,
          allowNameToggle,
          allowNameEdit: allowNameEdit ?? d.nameEditable ?? d.allowNameEdit,
          allowTypeEdit: allowTypeEdit ?? d.typeEditable ?? d.allowTypeEdit,
        });
        if (isEmptyVal(d.value))
          node.querySelector(".value").classList.add("placeholder", "muted");
        wrap.appendChild(node);
      });
    }
    return wrap;
  }

  function setHintContent(panel, message) {
    if (!panel) return;
    if (
      message &&
      typeof message === "object" &&
      Object.prototype.hasOwnProperty.call(message, "html")
    ) {
      panel.innerHTML = message.html || "";
    } else {
      panel.textContent = typeof message === "string" ? message : "";
    }
  }

  function resolveElement(ref) {
    if (!ref) return null;
    if (typeof ref === "string") {
      if (ref.startsWith("#") || ref.startsWith(".")) {
        return document.querySelector(ref);
      }
      return document.getElementById(ref);
    }
    return ref;
  }

  function createHintController({ button, panel, build } = {}) {
    const buttonEl = resolveElement(button);
    const panelEl = resolveElement(panel);

    function hide() {
      if (!panelEl) return;
      panelEl.textContent = "";
      panelEl.classList.add("hidden");
    }

    function show(message) {
      if (!panelEl) return;
      const content =
        typeof message === "undefined" && typeof build === "function"
          ? build()
          : message;
      setHintContent(panelEl, content ?? "");
      panelEl.classList.remove("hidden");
      flashStatus(panelEl);
    }

    if (buttonEl) {
      buttonEl.addEventListener("click", () => {
        show(typeof build === "function" ? build() : undefined);
      });
    }

    function setButtonHidden(hidden) {
      if (!buttonEl) return;
      buttonEl.classList.toggle("hidden", !!hidden);
    }

    return {
      button: buttonEl,
      panel: panelEl,
      show,
      hide,
      setButtonHidden,
    };
  }

  function createStepper({
    prefix,
    lines = [],
    nextPage = null,
    getBoundary,
    setBoundary,
    onBeforeChange,
    onAfterChange,
    isStepLocked,
    getStepBadge,
    endLabel,
  } = {}) {
    if (!prefix)
      throw new Error('createStepper requires a prefix (e.g., "p1").');
    const prevBtn = document.getElementById(`${prefix}-prev`);
    const nextBtn = document.getElementById(`${prefix}-next`);
    const total = Array.isArray(lines)
      ? lines.length
      : Math.max(0, Number(lines) || 0);

    function clearPulse() {
      nextBtn?.classList.remove("pulse-success");
    }

    function boundary() {
      return typeof getBoundary === "function" ? getBoundary() : 0;
    }

    function setBoundaryValue(value) {
      if (typeof setBoundary === "function") setBoundary(value);
    }

    function locked(at) {
      return typeof isStepLocked === "function"
        ? !!isStepLocked(at, at === total)
        : false;
    }

    function update() {
      const current = boundary();
      if (prevBtn) prevBtn.disabled = current === 0;
      if (nextBtn) {
        const atEnd = current === total;
        const badge =
          !atEnd && typeof getStepBadge === "function"
            ? getStepBadge(current + 1)
            : "";
        const badgeTag =
          badge === "note" ? "🔧" : badge === "check" ? "✅" : "";
        const isLocked = locked(current);
        const lockTag = isLocked ? " 🔒" : "";
        const labelPrefix = badgeTag ? `${badgeTag} ` : "";
        if (atEnd) {
          const label = endLabel || "Next Program";
          nextBtn.textContent = `${labelPrefix}${label}${lockTag} ▶▶`;
        } else {
          nextBtn.textContent = `${labelPrefix}Run line ${current + 1}${lockTag} ▶`;
        }
        nextBtn.disabled = isLocked;
      }
    }

    function sidebarParamValue() {
      return document.body.classList.contains("sidebar-collapsed") ? "0" : "1";
    }

    function withSidebarParam(url) {
      if (!url) return url;
      const [base, hash = ""] = url.split("#");
      const [path, query = ""] = base.split("?");
      const params = new URLSearchParams(query);
      params.set("sidebar", sidebarParamValue());
      const nextQuery = params.toString();
      const hashPart = hash ? `#${hash}` : "";
      return nextQuery
        ? `${path}?${nextQuery}${hashPart}`
        : `${path}${hashPart}`;
    }

    function goTo(target) {
      const current = boundary();
      const clamped = Math.max(0, Math.min(total, target));
      if (clamped === current) return;
      onBeforeChange?.(current);
      setBoundaryValue(clamped);
      onAfterChange?.(clamped);
      update();
    }

    prevBtn?.addEventListener("click", () => {
      if (boundary() === 0) return;
      clearPulse();
      goTo(boundary() - 1);
    });

    nextBtn?.addEventListener("click", () => {
      const current = boundary();
      clearPulse();
      if (current === total) {
        if (!nextBtn?.disabled && nextPage)
          window.location.href = withSidebarParam(nextPage);
        return;
      }
      if (locked(current)) return;
      goTo(current + 1);
    });

    update();

    return {
      update,
      goTo,
      boundary,
      clearPulse,
    };
  }

  function pulseNextButton(prefix) {
    const btn = document.getElementById(`${prefix}-next`);
    if (!btn) return;
    btn.classList.add("pulse-success");
  }

  document.addEventListener("focusin", (e) => {
    const t = e.target;
    if (
      t.classList?.contains("code-editable") &&
      t.classList.contains("placeholder")
    ) {
      const placeholder = t.dataset?.placeholder || "";
      if (txt(t) === placeholder) {
        t.textContent = "";
        t.classList.remove("placeholder", "muted");
      }
    }
    if (t.classList?.contains("placeholder")) {
      if (txt(t) === "") {
        t.classList.add("muted");
      } else {
        t.classList.remove("muted");
      }
    }
  });

  document.addEventListener("input", (e) => {
    const t = e.target;
    if (t.classList?.contains("placeholder")) {
      if (txt(t) === "") t.classList.add("muted");
      else t.classList.remove("muted");
    }
  });

  document.addEventListener("keydown", (e) => {
    if (e.key !== "Enter") return;
    const t = e.target;
    if (!t?.isContentEditable) return;
    if (
      t.classList?.contains("value") ||
      t.classList?.contains("type") ||
      t.classList?.contains("name-text")
    ) {
      e.preventDefault();
      t.blur();
    }
  });

  document.addEventListener("focusout", (e) => {
    const t = e.target;
    if (t.classList?.contains("code-editable")) {
      const placeholder = t.dataset?.placeholder || "";
      if (!txt(t)) {
        t.textContent = placeholder;
        if (placeholder) t.classList.add("placeholder", "muted");
      }
    }
    if (t.classList?.contains("placeholder") && txt(t) === "") {
      t.textContent = "";
      t.classList.add("muted");
    }
  });

  function initScrollHint() {
    const btn = el(
      '<button class="scroll-down-btn hidden" aria-label="Scroll to bottom">↓</button>',
    );
    document.body.appendChild(btn);

    const shouldShow = () => {
      const doc = document.documentElement;
      const scrollable = doc.scrollHeight - window.innerHeight;
      const nearBottom = window.scrollY > scrollable - 140;
      return scrollable > 200 && !nearBottom;
    };

    const update = () => {
      btn.classList.toggle("hidden", !shouldShow());
    };

    window.addEventListener("scroll", update, { passive: true });
    window.addEventListener("resize", update);
    const observer = new MutationObserver(update);
    observer.observe(document.body, { childList: true, subtree: true });
    btn.addEventListener("click", () => {
      window.scrollTo({
        top: document.documentElement.scrollHeight,
        behavior: "smooth",
      });
    });
    update();
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", initScrollHint);
  } else {
    initScrollHint();
  }

  function initInstructionWatcher() {
    const seen = new Map();
    document.querySelectorAll(".intro").forEach((el) => {
      seen.set(el, el.innerHTML);
      const obs = new MutationObserver(() => {
        const prev = seen.get(el) || "";
        const curr = el.innerHTML;
        if (curr === prev) return;
        seen.set(el, curr);
        const text = (el.textContent || "").trim();
        if (text) {
          window.scrollTo({ top: 0, behavior: "smooth" });
        }
      });
      obs.observe(el, { childList: true, characterData: true, subtree: true });
    });
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", initInstructionWatcher, {
      once: true,
    });
  } else {
    initInstructionWatcher();
  }

  function applySidebarStateFromUrl() {
    const params = new URLSearchParams(window.location.search);
    const state = params.get("sidebar");
    if (state === "0") document.body.classList.add("sidebar-collapsed");
    if (state === "1") document.body.classList.remove("sidebar-collapsed");
  }

  applySidebarStateFromUrl();

  function initSidebarToggle() {
    const wrap = document.querySelector(".wrap");
    const nav = wrap?.querySelector("nav");
    if (!wrap || !nav) return;
    if (!nav.id) nav.id = "sidebar";
    let btn = document.querySelector(".sidebar-toggle");
    if (!btn) {
      btn = el(
        '<button type="button" class="sidebar-toggle"><span class="hamburger" aria-hidden="true"><span></span><span></span><span></span></span><span class="sr-only">Toggle sidebar</span></button>',
      );
      document.body.appendChild(btn);
    }
    btn.setAttribute("aria-controls", nav.id);
    const updateLabel = () => {
      const hidden = document.body.classList.contains("sidebar-collapsed");
      const label = hidden ? "Show sidebar" : "Hide sidebar";
      btn.setAttribute("aria-label", label);
      btn.setAttribute("aria-expanded", hidden ? "false" : "true");
      const sr = btn.querySelector(".sr-only");
      if (sr) sr.textContent = label;
    };
    const updateUrl = () => {
      const hidden = document.body.classList.contains("sidebar-collapsed");
      const params = new URLSearchParams(window.location.search);
      params.set("sidebar", hidden ? "0" : "1");
      const query = params.toString();
      const next = `${window.location.pathname}?${query}${window.location.hash}`;
      window.history.replaceState(null, "", next);
    };
    btn.addEventListener("click", () => {
      document.body.classList.toggle("sidebar-collapsed");
      updateLabel();
      updateUrl();
    });
    updateLabel();
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", initSidebarToggle, {
      once: true,
    });
  } else {
    initSidebarToggle();
  }

  function flashStatus(el) {
    if (!el) return;
    el.classList.remove("status-flash");
    // force reflow to restart animation
    void el.offsetWidth;
    el.classList.add("status-flash");
  }

  global.MB = {
    $,
    el,
    randAddr,
    isEmptyVal,
    txt,
    renderCodePane,
    renderCodePaneEditable,
    readEditableCodeLines,
    parseSimpleStatement,
    applySimpleStatement,
    createSimpleSimulator,
    vbox,
    readBoxState,
    makeAnswerBox,
    cloneStateBoxes,
    ensureBox,
    cloneBoxes,
    firstNonEmptyClone,
    serializeWorkspace,
    restoreWorkspace,
    setHintContent,
    createHintController,
    createStepper,
    pulseNextButton,
    flashStatus,
    disableBoxEditing,
    removeBoxDeleteButtons,
  };
})(window);
