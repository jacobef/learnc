(function (global) {
  function $(selector, root = document) {
    return root.querySelector(selector);
  }

  function el(html) {
    const t = document.createElement("template");
    t.innerHTML = html.trim();
    return t.content.firstElementChild;
  }

  function parseType(type = "int") {
    const clean = String(type || "").trim();
    const match = clean.match(/^(int|long)(\*+)?$/);
    if (!match) return { base: null, depth: null };
    const base = match[1];
    const depth = match[2] ? match[2].length : 0;
    return { base, depth };
  }

  function getPointerDepth(type = "int") {
    const { depth } = parseType(type);
    return depth;
  }

  function normalizeNamesList(box) {
    if (!box) return [];
    const baseName =
      box.name !== undefined && box.name !== null ? String(box.name) : "";
    let names = Array.isArray(box.names) ? [...box.names] : [];
    names = names.filter((n) => n !== undefined && n !== null);
    if (baseName) {
      names = names.filter((n) => n !== baseName);
      names.unshift(baseName);
    }
    return names;
  }

  function walkPointerAliasChain(boxes, startAddr, depth, cb) {
    if (!Array.isArray(boxes) || !boxes.length) return;
    if (!Number.isFinite(depth) || depth < 1) return;
    if (startAddr == null || isEmptyVal(String(startAddr))) return;
    let current = boxes.find(
      (b) => String(b.address ?? "") === String(startAddr),
    );
    for (let level = 1; level <= depth; level++) {
      if (!current) break;
      cb(current, level);
      if (level === depth) break;
      const nextAddr = current.value;
      if (nextAddr == null || isEmptyVal(String(nextAddr))) break;
      current = boxes.find(
        (b) => String(b.address ?? "") === String(nextAddr),
      );
    }
  }

  function removePointerAliasesFromValue(boxes, ptrName, ptrType, ptrValue) {
    if (!ptrName) return;
    const depth = getPointerDepth(ptrType);
    walkPointerAliasChain(boxes, ptrValue, depth, (box, level) => {
      const alias = `${"*".repeat(level)}${ptrName}`;
      const names = normalizeNamesList(box).filter((n) => n !== alias);
      box.names = names;
    });
  }

  function addPointerAliasesFromValue(boxes, ptrName, ptrType, ptrValue) {
    if (!ptrName) return;
    const depth = getPointerDepth(ptrType);
    walkPointerAliasChain(boxes, ptrValue, depth, (box, level) => {
      const alias = `${"*".repeat(level)}${ptrName}`;
      const names = normalizeNamesList(box);
      if (!names.includes(alias)) names.push(alias);
      box.names = names;
    });
  }

  function refreshPointerAliases(boxes, target, oldValue) {
    if (!target) return;
    const depth = getPointerDepth(target.type);
    if (!Number.isFinite(depth) || depth < 1) return;
    removePointerAliasesFromValue(boxes, target.name, target.type, oldValue);
    addPointerAliasesFromValue(boxes, target.name, target.type, target.value);
  }

  function typeInfo(type = "int") {
    const { base, depth } = parseType(type);
    if (!base) return { size: 4, align: 4 };
    if (depth > 0) return { size: 8, align: 8 };
    if (base === "long") return { size: 8, align: 8 };
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

  function normalizeZeroDisplay(value) {
    const trimmed = String(value ?? "").trim();
    if (trimmed === "-0") return "0";
    return trimmed;
  }

  function txt(n) {
    return (n?.textContent || "").trim();
  }

  function disableAutoText(el) {
    if (!el || el.nodeType !== 1) return;
    el.setAttribute("autocapitalize", "off");
    el.setAttribute("autocorrect", "off");
    el.setAttribute("autocomplete", "off");
    el.setAttribute("spellcheck", "false");
  }

  function applyAutoTextDefaults(root = document) {
    if (!root) return;
    root
      .querySelectorAll(
        'input[type="text"], input:not([type]), textarea, [contenteditable="true"]',
      )
      .forEach((el) => disableAutoText(el));
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
        disableAutoText(src);
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
    let m = s.match(
      /^(int|long)\b\s*(\*{0,2})\s*([A-Za-z_][A-Za-z0-9_]*)\s*;$/,
    );
    if (m) {
      const base = m[1];
      const stars = m[2] || "";
      const type =
        stars === "**"
          ? `${base}**`
          : stars === "*"
            ? `${base}*`
            : base;
      return { kind: "decl", name: m[3], type };
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
        const { base, depth } = parseType(target.type);
        if (!base || depth !== 0) return null;
        target.value = String(stmt.value);
        return boxes;
      }
      // assignRef
      if (!isPointerType(target.type)) return null;
      const refBox = by[stmt.ref];
      if (!refBox || !refBox.address) return null;
      if (!isRefCompatible(target.type, refBox.type)) return null;
      const oldValue = target.value;
      target.value = String(refBox.address);
      refreshPointerAliases(boxes, target, oldValue);
      return boxes;
    }
    if (stmt.kind === "assignDeref") {
      const ptr = by[stmt.name];
      const { base, depth } = parseType(ptr?.type);
      if (
        !ptr ||
        !base ||
        depth !== 1 ||
        !ptr.value ||
        ptr.value === "empty"
      )
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
            type: ident === "int" || ident === "long" ? "kw" : "ident",
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

    function resolveDeclType(stars, baseType = "int") {
      if (!Number.isFinite(stars) || stars < 0) return null;
      if (!baseType || (baseType !== "int" && baseType !== "long")) return null;
      if (stars === 0) return baseType;
      if (!allowPointers) return null;
      return `${baseType}${"*".repeat(stars)}`;
    }

    function isPointerType(type) {
      const { depth } = parseType(type);
      return Number.isFinite(depth) && depth > 0;
    }

    function pointerDepth(type) {
      const { depth } = parseType(type);
      return depth;
    }

    function makePointerType(depth, base = "int") {
      if (!Number.isFinite(depth) || depth < 0) return null;
      if (base !== "int" && base !== "long") return null;
      return depth === 0 ? base : `${base}${"*".repeat(depth)}`;
    }

    const INT32_MIN = -2147483648n;
    const INT32_MAX = 2147483647n;
    const INT64_MIN = -9223372036854775808n;
    const INT64_MAX = 9223372036854775807n;

    function classifyNumericLiteral(value) {
      try {
        const n = BigInt(String(value));
        if (n < INT64_MIN || n > INT64_MAX) return "compile";
        if (n < INT32_MIN || n > INT32_MAX) return "ub";
        return "ok";
      } catch {
        return "compile";
      }
    }

    function numericLiteralErrorForType(value, type) {
      const { base, depth } = parseType(type);
      if (!base || depth !== 0) return null;
      const status = classifyNumericLiteral(value);
      if (base === "long") {
        return status === "compile" ? numericLiteralError(status) : null;
      }
      if (base === "int") return numericLiteralError(status);
      return null;
    }

    function numericLiteralError(kind) {
      if (kind === "compile")
        return {
          error: "That number is too large to represent.",
          kind: "compile",
        };
      if (kind === "ub")
        return { error: "That number is too large for int.", kind: "ub" };
      return null;
    }

    function isRefCompatible(targetType, refType) {
      const { base: targetBase, depth: targetDepth } = parseType(targetType);
      const { base: refBase, depth: refDepth } = parseType(refType);
      if (
        !targetBase ||
        !refBase ||
        !Number.isFinite(targetDepth) ||
        !Number.isFinite(refDepth)
      )
        return false;
      return targetBase === refBase && targetDepth === refDepth + 1;
    }

    function expectedPointerTypeForRef(refType) {
      const { base, depth } = parseType(refType);
      if (!base || !Number.isFinite(depth)) return null;
      return makePointerType(depth + 1, base);
    }

    function isDeclPrefix(tokens) {
      if (!tokens.length) return false;
      if (tokens[0].type !== "kw") return false;
      const baseType = tokens[0].value;
      if (baseType !== "int" && baseType !== "long") return false;
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
      if (!resolveDeclType(stars, baseType)) return false;
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
        if (!isPointerType(resolveDeclType(stars, baseType))) return false;
        if (idx === tokens.length - 1) return true;
        return tokens[idx + 1]?.type === "ident" && idx + 2 === tokens.length;
      }
      if (
        rhs.type === "sym" &&
        (rhs.value === "*" || rhs.value === "&") &&
        allowPointers
      ) {
        if (!allowDeclAssignVar) return false;
        let j = idx;
        while (
          j < tokens.length &&
          tokens[j].type === "sym" &&
          (tokens[j].value === "*" || tokens[j].value === "&")
        )
          j++;
        if (j === tokens.length) return true;
        return tokens[j].type === "ident" && j === tokens.length - 1;
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
      if (
        t2.type === "sym" &&
        (t2.value === "&" || t2.value === "*") &&
        allowPointers
      ) {
        let j = 2;
        while (
          j < tokens.length &&
          tokens[j].type === "sym" &&
          (tokens[j].value === "*" || tokens[j].value === "&")
        )
          j++;
        if (j === tokens.length) return true;
        if (tokens[j].type !== "ident") return false;
        return (
          j === tokens.length - 1 &&
          hasDeclaredPrefix(tokens[j].value, declaredNames)
        );
      }
      return false;
    }

    function isUnaryAssignPrefix(tokens, declaredNames) {
      if (!allowPointers) return false;
      if (!tokens.length) return false;
      let idx = 0;
      while (
        idx < tokens.length &&
        tokens[idx].type === "sym" &&
        (tokens[idx].value === "*" || tokens[idx].value === "&")
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
      if (
        rhs.type === "sym" &&
        (rhs.value === "&" || rhs.value === "*") &&
        allowPointers
      ) {
        let j = idx;
        while (
          j < tokens.length &&
          tokens[j].type === "sym" &&
          (tokens[j].value === "*" || tokens[j].value === "&")
        )
          j++;
        if (j === tokens.length) return true;
        if (tokens[j].type !== "ident") return false;
        return (
          j === tokens.length - 1 &&
          hasDeclaredPrefix(tokens[j].value, declaredNames)
        );
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
        if (t0.type === "kw" && (t0.value === "int" || t0.value === "long"))
          return true;
        if (t0.type === "ident") {
          if (
            allowIntPrefix &&
            ("int".startsWith(t0.value) || "long".startsWith(t0.value))
          )
            return true;
          return hasDeclaredPrefix(t0.value, declaredNames);
        }
        if (allowPointers && t0.type === "sym" && t0.value === "*") return true;
      }
      return (
        isDeclPrefix(tokens) ||
        isAssignPrefix(tokens, declaredNames) ||
        isDerefPrefix(tokens, declaredNames) ||
        isUnaryAssignPrefix(tokens, declaredNames)
      );
    }

    function parseAssignRhs(tokens, idx) {
      if (idx >= tokens.length) return null;
      const rhs = tokens[idx];
      if (rhs.type === "number" && idx === tokens.length - 1) {
        return { kind: "num", value: rhs.value };
      }
      if (rhs.type === "ident" && allowVarAssign && idx === tokens.length - 1) {
        return { kind: "var", name: rhs.value };
      }
      if (rhs.type === "sym" && rhs.value === "&" && allowPointers) {
        const next = tokens[idx + 1];
        if (next?.type === "ident" && idx + 2 === tokens.length) {
          return { kind: "ref", name: next.value };
        }
      }
      if (
        rhs.type === "sym" &&
        (rhs.value === "*" || rhs.value === "&") &&
        allowPointers
      ) {
        let j = idx;
        let depth = 0;
        while (
          j < tokens.length &&
          tokens[j].type === "sym" &&
          (tokens[j].value === "*" || tokens[j].value === "&")
        ) {
          depth++;
          j++;
        }
        if (
          depth > 0 &&
          tokens[j]?.type === "ident" &&
          j + 1 === tokens.length
        ) {
          const ops = tokens.slice(idx, j).map((tok) => String(tok.value));
          if (ops.every((op) => op === "*")) {
            return { kind: "deref", name: tokens[j].value, depth };
          }
          return { kind: "unary", name: tokens[j].value, ops };
        }
      }
      return null;
    }

    function parseUnaryLhs(tokens) {
      if (!allowPointers) return null;
      if (!tokens.length) return null;
      let idx = 0;
      const ops = [];
      while (
        idx < tokens.length &&
        tokens[idx].type === "sym" &&
        (tokens[idx].value === "*" || tokens[idx].value === "&")
      ) {
        ops.push(tokens[idx].value);
        idx++;
      }
      if (!ops.length) return null;
      if (idx >= tokens.length || tokens[idx].type !== "ident") return null;
      const name = tokens[idx].value;
      return { ops, name, idx: idx + 1 };
    }

    function parseStatementTokens(tokens) {
      if (!tokens.length) return null;
      if (tokens[0].type === "kw") {
        const baseType = tokens[0].value;
        if (baseType !== "int" && baseType !== "long") return null;
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
        const declType = resolveDeclType(stars, baseType);
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
        if (allowPointers && allowDeclAssignVar && rhs.type === "sym") {
          let j = idx;
          let depth = 0;
          while (
            j < tokens.length &&
            tokens[j].type === "sym" &&
            tokens[j].value === "*"
          ) {
            depth++;
            j++;
          }
          if (depth > 0 && tokens[j]?.type === "ident" && j + 1 === tokens.length) {
            return {
              kind: "declAssignDeref",
              name,
              ptr: tokens[j].value,
              depth,
              declType,
            };
          }
        }
        if (allowPointers && allowDeclAssignVar && rhs.type === "sym") {
          let j = idx;
          const ops = [];
          while (
            j < tokens.length &&
            tokens[j].type === "sym" &&
            (tokens[j].value === "*" || tokens[j].value === "&")
          ) {
            ops.push(tokens[j].value);
            j++;
          }
          if (
            ops.length > 0 &&
            ops.includes("&") &&
            tokens[j]?.type === "ident" &&
            j + 1 === tokens.length
          ) {
            return {
              kind: "declAssignUnary",
              name,
              src: tokens[j].value,
              ops,
              declType,
            };
          }
        }
        return null;
      }
      const unary = parseUnaryLhs(tokens);
      if (unary) {
        let idx = unary.idx;
        if (
          idx >= tokens.length ||
          tokens[idx].type !== "sym" ||
          tokens[idx].value !== "="
        )
          return null;
        idx++;
        const rhs = parseAssignRhs(tokens, idx);
        if (!rhs) return null;
        return { kind: "assignUnary", name: unary.name, ops: unary.ops, rhs };
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
      if (
        tokens.length >= 4 &&
        tokens[0].type === "ident" &&
        tokens[1].type === "sym" &&
        tokens[1].value === "=" &&
        tokens[2].type === "sym" &&
        tokens[2].value === "*" &&
        allowPointers
      ) {
        let j = 2;
        let depth = 0;
        while (
          j < tokens.length &&
          tokens[j].type === "sym" &&
          tokens[j].value === "*"
        ) {
          depth++;
          j++;
        }
        if (depth > 0 && tokens[j]?.type === "ident" && j + 1 === tokens.length) {
          return {
            kind: "assignFromDeref",
            name: tokens[0].value,
            ptr: tokens[j].value,
            depth,
          };
        }
      }
      if (
        allowPointers &&
        tokens.length >= 4 &&
        tokens[0].type === "ident" &&
        tokens[1].type === "sym" &&
        tokens[1].value === "=" &&
        tokens[2].type === "sym" &&
        (tokens[2].value === "*" || tokens[2].value === "&")
      ) {
        let j = 2;
        const ops = [];
        while (
          j < tokens.length &&
          tokens[j].type === "sym" &&
          (tokens[j].value === "*" || tokens[j].value === "&")
        ) {
          ops.push(tokens[j].value);
          j++;
        }
        if (
          ops.length > 0 &&
          ops.includes("&") &&
          tokens[j]?.type === "ident" &&
          j + 1 === tokens.length
        ) {
          return {
            kind: "assignUnaryRhs",
            name: tokens[0].value,
            src: tokens[j].value,
            ops,
          };
        }
      }
      return null;
    }

    function parseDeclAssignRefFallback(tokens) {
      if (!allowPointers) return null;
      if (!tokens.length) return null;
      if (tokens[0].type !== "kw") return null;
      const baseType = tokens[0].value;
      if (baseType !== "int" && baseType !== "long") return null;
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
      const declType = resolveDeclType(stars, baseType);
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
      const { base: targetBase, depth: targetDepth } = parseType(target.type);
      const { base: sourceBase, depth: sourceDepth } = parseType(source.type);
      const sameType = target.type === source.type;
      const isScalar =
        targetBase &&
        sourceBase &&
        targetBase === sourceBase &&
        targetDepth === 0 &&
        sourceDepth === 0;
      const isPtr = sameType && isPointerType(target.type);
      if (!isScalar && !isPtr) return null;
      const oldValue = target.value;
      target.value = String(source.value ?? "empty");
      if (isPtr) {
        refreshPointerAliases(boxes, target, oldValue);
      }
      return boxes;
    }

    function applyAssignRef(state, stmt) {
      const boxes = cloneBoxes(state);
      const by = Object.fromEntries(boxes.map((b) => [b.name, b]));
      const target = by[stmt.name];
      const refBox = by[stmt.ref];
      if (!target || !refBox || !refBox.address) return null;
      if (!isRefCompatible(target.type, refBox.type)) return null;
      const oldValue = target.value;
      target.value = String(refBox.address);
      refreshPointerAliases(boxes, target, oldValue);
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
      const { base: targetBase, depth: targetDepth } = parseType(target.type);
      if (!Number.isFinite(targetDepth)) return null;
      const oldValue = target.value;
      if (stmt.kind === "assignDeref") {
        if (targetDepth !== 0) return null;
        target.value = String(stmt.value);
      } else if (stmt.kind === "assignDerefVar") {
        const by = Object.fromEntries(boxes.map((b) => [b.name, b]));
        const source = by[stmt.src];
        if (!source) return null;
        if (requireSourceValue && isEmptyVal(source.value ?? "")) return null;
        const { base: sourceBase, depth: sourceDepth } = parseType(source.type);
        if (
          !sourceBase ||
          !targetBase ||
          sourceBase !== targetBase ||
          sourceDepth !== targetDepth
        )
          return null;
        target.value = String(source.value ?? "empty");
      } else if (stmt.kind === "assignDerefRef") {
        const by = Object.fromEntries(boxes.map((b) => [b.name, b]));
        const refBox = by[stmt.ref];
        if (!refBox || !refBox.address) return null;
        if (!isRefCompatible(target.type, refBox.type)) return null;
        target.value = String(refBox.address);
      }
      if (targetDepth > 0) {
        refreshPointerAliases(boxes, target, oldValue);
      }
      return boxes;
    }

    function applyAssignDerefVar(state, stmt) {
      return applyAssignDeref(state, stmt);
    }

    function resolveUnaryLvalue(state, ops, name) {
      const by = Object.fromEntries(state.map((b) => [b.name, b]));
      const base = by[name];
      if (!base) return { error: "missing", name };
      let current = {
        kind: "lvalue",
        type: base.type,
        value: base.value,
        address: base.address ?? "",
        box: base,
      };
      let label = name;
      for (let i = ops.length - 1; i >= 0; i--) {
        const op = ops[i];
        if (op === "&") {
          const nextLabel = `&${label}`;
          if (current.kind !== "lvalue" || !current.address) {
            return { error: "not_lvalue", label: nextLabel };
          }
          const { base, depth } = parseType(current.type);
          const nextDepth = Number.isFinite(depth) ? depth + 1 : 1;
          current = {
            kind: "rvalue",
            type: makePointerType(nextDepth, base || "int") || "int*",
            value: String(current.address),
            address: "",
            box: null,
          };
          label = nextLabel;
          continue;
        }
        if (op === "*") {
          const nextLabel = `*${label}`;
          const { base, depth } = parseType(current.type);
          if (!Number.isFinite(depth) || depth < 1) {
            return { error: "not_deref", label: nextLabel };
          }
          const ptrVal = String(current.value ?? "").trim();
          if (!ptrVal || /^empty$/i.test(ptrVal)) {
            return { error: "empty", label };
          }
          const target = state.find(
            (b) => String(b.address ?? "") === String(ptrVal),
          );
          if (!target) return { error: "unknown", label: nextLabel };
          current = {
            kind: "lvalue",
            type: makePointerType(depth - 1, base || "int") || "int",
            value: target.value,
            address: target.address ?? "",
            box: target,
          };
          label = nextLabel;
        }
      }
      if (current.kind !== "lvalue" || !current.box) {
        return { error: "not_lvalue", label };
      }
      return { target: current.box, label, type: current.type };
    }

    function resolveUnaryExpr(state, ops, name) {
      const by = Object.fromEntries(state.map((b) => [b.name, b]));
      const base = by[name];
      if (!base) return { error: "missing", name };
      let current = {
        kind: "lvalue",
        type: base.type,
        value: base.value,
        address: base.address ?? "",
        box: base,
        refBox: null,
      };
      let label = name;
      for (let i = ops.length - 1; i >= 0; i--) {
        const op = ops[i];
        if (op === "&") {
          const nextLabel = `&${label}`;
          if (current.kind !== "lvalue" || !current.address) {
            return { error: "not_lvalue", label: nextLabel };
          }
          const { base, depth } = parseType(current.type);
          const nextDepth = Number.isFinite(depth) ? depth + 1 : 1;
          current = {
            kind: "rvalue",
            type: makePointerType(nextDepth, base || "int") || "int*",
            value: String(current.address),
            address: "",
            box: null,
            refBox: current.box,
          };
          label = nextLabel;
          continue;
        }
        if (op === "*") {
          const nextLabel = `*${label}`;
          const { base, depth } = parseType(current.type);
          if (!Number.isFinite(depth) || depth < 1) {
            return { error: "not_deref", label: nextLabel };
          }
          const ptrVal = String(current.value ?? "").trim();
          if (!ptrVal || /^empty$/i.test(ptrVal)) {
            return { error: "empty", label };
          }
          const target = state.find(
            (b) => String(b.address ?? "") === String(ptrVal),
          );
          if (!target) return { error: "unknown", label: nextLabel };
          current = {
            kind: "lvalue",
            type: makePointerType(depth - 1, base || "int") || "int",
            value: target.value,
            address: target.address ?? "",
            box: target,
            refBox: null,
          };
          label = nextLabel;
        }
      }
      return { result: current, label };
    }

    function minBaseDepthForOps(ops) {
      let delta = 0;
      let required = 0;
      for (let i = ops.length - 1; i >= 0; i--) {
        const op = ops[i];
        if (op === "&") {
          delta += 1;
        } else if (op === "*") {
          required = Math.max(required, 1 - delta);
          delta -= 1;
        }
      }
      return Math.max(0, required);
    }

    function validateUnaryRhs(state, targetType, targetName, ops, src) {
      const by = Object.fromEntries(state.map((b) => [b.name, b]));
      const base = by[src];
      const minDepth = minBaseDepthForOps(ops || []);
      const { base: srcBase } = parseType(base?.type);
      const requiredBaseType =
        makePointerType(minDepth, srcBase || "int") ||
        `int${"*".repeat(minDepth)}`;
      if (!base) {
        return {
          error: `You can't use ${src} before declaring it.`,
          kind: "compile",
        };
      }
      const baseDepth = pointerDepth(base.type);
      if (!Number.isFinite(baseDepth) || baseDepth < minDepth) {
        return typeMismatchError(src, requiredBaseType);
      }
      const resolved = resolveUnaryExpr(state, ops, src);
      if (resolved?.error === "empty") {
        return {
          error: `${resolved.label} doesn't have a value yet, so it can't be dereferenced.`,
          kind: "ub",
        };
      }
      if (resolved?.error === "unknown") {
        return {
          error: `${resolved.label} doesn't point to a known variable.`,
          kind: "ub",
        };
      }
      if (resolved?.error === "not_deref") {
        return {
          error: `${resolved.label} is not a valid dereference.`,
          kind: "compile",
        };
      }
      if (resolved?.error === "not_lvalue") {
        return { error: "That assignment is not valid here.", kind: "compile" };
      }
      const result = resolved?.result;
      if (!result) {
        return { error: "That assignment is not valid here.", kind: "compile" };
      }
      const { base: targetBase, depth: targetDepth } = parseType(targetType);
      const { base: resultBase, depth: resultDepth } = parseType(result.type);
      if (
        !targetBase ||
        !resultBase ||
        !Number.isFinite(targetDepth) ||
        !Number.isFinite(resultDepth)
      ) {
        return { error: "That assignment is not valid here.", kind: "compile" };
      }
      if (targetBase !== resultBase || targetDepth !== resultDepth) {
        return typeMismatchError(
          targetName,
          makePointerType(resultDepth, resultBase) ||
            `int${"*".repeat(resultDepth)}`,
        );
      }
      if (result.kind === "lvalue") {
        if (requireSourceValue && isEmptyVal(result.value ?? "")) {
          return {
            error: `${resolved.label} doesn't have a value yet.`,
            kind: "ub",
          };
        }
      }
      return null;
    }

    function applyAssignUnary(state, stmt) {
      const boxes = cloneBoxes(state);
      const resolved = resolveUnaryLvalue(boxes, stmt.ops, stmt.name);
      if (!resolved?.target) return null;
      const targetName = resolved.target.name;
      if (stmt.rhs.kind === "num") {
        return applyStatement(
          boxes,
          {
            kind: "assign",
            name: targetName,
            value: stmt.rhs.value,
            valueKind: "num",
          },
          {},
        );
      }
      if (stmt.rhs.kind === "var") {
        return applyAssignVar(boxes, {
          kind: "assignVar",
          name: targetName,
          src: stmt.rhs.name,
        });
      }
      if (stmt.rhs.kind === "ref") {
        return applyAssignRef(boxes, {
          kind: "assignRef",
          name: targetName,
          ref: stmt.rhs.name,
        });
      }
      if (stmt.rhs.kind === "deref") {
        return applyAssignFromDeref(boxes, {
          kind: "assignFromDeref",
          name: targetName,
          ptr: stmt.rhs.name,
          depth: stmt.rhs.depth,
        });
      }
      if (stmt.rhs.kind === "unary") {
        return applyAssignUnaryRhs(boxes, {
          kind: "assignUnaryRhs",
          name: targetName,
          src: stmt.rhs.name,
          ops: stmt.rhs.ops,
        });
      }
      return null;
    }

    function applyAssignUnaryRhs(state, stmt) {
      const boxes = cloneBoxes(state);
      const by = Object.fromEntries(boxes.map((b) => [b.name, b]));
      const target = by[stmt.name];
      if (!target) return null;
      const resolved = resolveUnaryExpr(boxes, stmt.ops, stmt.src);
      if (!resolved?.result) return null;
      const result = resolved.result;
      const { base: targetBase, depth: targetDepth } = parseType(target.type);
      const { base: resultBase, depth: resultDepth } = parseType(result.type);
      if (
        !targetBase ||
        !resultBase ||
        !Number.isFinite(targetDepth) ||
        !Number.isFinite(resultDepth)
      )
        return null;
      if (targetBase !== resultBase || targetDepth !== resultDepth) return null;
      const value =
        result.kind === "lvalue" ? result.box?.value : result.value;
      if (requireSourceValue && isEmptyVal(value ?? "")) return null;
      const oldValue = target.value;
      target.value = String(value ?? "empty");
      if (targetDepth > 0) {
        refreshPointerAliases(boxes, target, oldValue);
      }
      return boxes;
    }

    function applyAssignFromDeref(state, stmt) {
      const boxes = cloneBoxes(state);
      const by = Object.fromEntries(boxes.map((b) => [b.name, b]));
      const target = by[stmt.name];
      if (!target) return null;
      const { target: source } = resolveDerefTarget(
        boxes,
        stmt.ptr,
        stmt.depth || 1,
      );
      if (!source) return null;
      const { base: targetBase, depth: targetDepth } = parseType(target.type);
      const { base: sourceBase, depth: sourceDepth } = parseType(source.type);
      if (
        !targetBase ||
        !sourceBase ||
        !Number.isFinite(targetDepth) ||
        !Number.isFinite(sourceDepth)
      )
        return null;
      if (targetBase !== sourceBase || targetDepth !== sourceDepth) return null;
      if (requireSourceValue && isEmptyVal(source.value ?? "")) return null;
      const oldValue = target.value;
      target.value = String(source.value ?? "empty");
      if (targetDepth > 0) {
        refreshPointerAliases(boxes, target, oldValue);
      }
      return boxes;
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
      if (stmt.kind === "declAssignDeref") {
        const decl = {
          kind: "decl",
          name: stmt.name,
          type: stmt.declType || "int",
        };
        const afterDecl = applySimpleStatement(state, decl, opts);
        if (!afterDecl) return null;
        return applyAssignFromDeref(afterDecl, stmt);
      }
      if (stmt.kind === "declAssignUnary") {
        const decl = {
          kind: "decl",
          name: stmt.name,
          type: stmt.declType || "int",
        };
        const afterDecl = applySimpleStatement(state, decl, opts);
        if (!afterDecl) return null;
        const assignUnaryRhs = {
          kind: "assignUnaryRhs",
          name: stmt.name,
          src: stmt.src,
          ops: stmt.ops,
        };
        return applyAssignUnaryRhs(afterDecl, assignUnaryRhs);
      }
      if (stmt.kind === "assignVar") {
        return applyAssignVar(state, stmt);
      }
      if (stmt.kind === "assignRef") {
        return applyAssignRef(state, stmt);
      }
      if (stmt.kind === "assignUnary") {
        return applyAssignUnary(state, stmt);
      }
      if (stmt.kind === "assignUnaryRhs") {
        return applyAssignUnaryRhs(state, stmt);
      }
      if (stmt.kind === "assignFromDeref") {
        return applyAssignFromDeref(state, stmt);
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
      if (tokens[0].type === "kw") {
        const baseType = tokens[0].value;
        if (baseType !== "int" && baseType !== "long")
          return "A declaration needs a variable name.";
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
            return `Pointer declarations should assign from an address, like "${baseType}* name = &x;".`;
          }
          return "A declaration needs a variable name.";
        }
        if (tokens[1].type !== "ident")
          return "A declaration needs a variable name.";
        if (allowDeclAssign || allowDeclAssignVar)
          return 'Declarations should look like "int name;" or "long name;" or "int name = value;".';
        return 'Declarations should look like "int name;" or "long name;".';
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
        parsed.kind === "declAssignRef" ||
        parsed.kind === "declAssignDeref" ||
        parsed.kind === "declAssignUnary"
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
        if (parsed.kind === "declAssign" && parsed.valueKind === "num") {
          const err = numericLiteralErrorForType(
            parsed.value,
            parsed.declType || "int",
          );
          if (err) return err;
        }
        if (parsed.kind === "declAssignDeref") {
          const by = Object.fromEntries(state.map((b) => [b.name, b]));
          const ptr = by[parsed.ptr];
          if (!ptr) {
            return {
              error: `You can't use ${parsed.ptr} before declaring it.`,
              kind: "compile",
            };
          }
          const depth = parsed.depth || 1;
          const { base: ptrBase, depth: ptrDepth } = parseType(ptr.type);
          const { base: declBase, depth: declDepth } = parseType(
            parsed.declType || "int",
          );
          if (!ptrBase || !declBase) {
            return typeMismatchError(
              parsed.ptr,
              makePointerType(depth) || `int${"*".repeat(depth)}`,
            );
          }
          if (!Number.isFinite(ptrDepth) || ptrDepth < depth) {
            return typeMismatchError(
              parsed.ptr,
              makePointerType(depth, ptrBase) || `int${"*".repeat(depth)}`,
            );
          }
          const resultDepth = ptrDepth - depth;
          if (declBase !== ptrBase || declDepth !== resultDepth) {
            return typeMismatchError(
              parsed.name,
              makePointerType(resultDepth, ptrBase) ||
                `int${"*".repeat(resultDepth)}`,
            );
          }
          let current = ptr;
          const derefLabel = `${"*".repeat(depth)}${parsed.ptr}`;
          for (let i = 0; i < depth; i++) {
            if (!isPointerType(current.type)) {
              return typeMismatchError(
                parsed.ptr,
                makePointerType(depth, ptrBase) || `int${"*".repeat(depth)}`,
              );
            }
            if (!current.value || current.value === "empty") {
              return {
                error: `${derefLabel} doesn't have a value yet.`,
                kind: "ub",
              };
            }
            const next = state.find((b) => b.address === String(current.value));
            if (!next) {
              return {
                error: `${parsed.ptr} doesn't point to a known variable.`,
                kind: "ub",
              };
            }
            current = next;
          }
          if (requireSourceValue && isEmptyVal(current.value ?? "")) {
            return {
              error: `${derefLabel} doesn't have a value yet.`,
              kind: "ub",
            };
          }
        }
        if (parsed.kind === "declAssignUnary") {
          const err = validateUnaryRhs(
            state,
            parsed.declType || "int",
            parsed.name,
            parsed.ops,
            parsed.src,
          );
          if (err) return err;
        }
      } else if (parsed.kind === "assign") {
        if (!seenDecl.has(parsed.name)) {
          return missingDeclError(parsed.name, "int");
        }
        const by = Object.fromEntries(state.map((b) => [b.name, b]));
        const err = numericLiteralErrorForType(
          parsed.value,
          by[parsed.name]?.type || "int",
        );
        if (err) return err;
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
      } else if (parsed.kind === "assignUnary") {
        const by = Object.fromEntries(state.map((b) => [b.name, b]));
        const base = by[parsed.name];
        const minDepth = minBaseDepthForOps(parsed.ops || []);
        const { base: baseType } = parseType(base?.type);
        const requiredBaseType =
          makePointerType(minDepth, baseType || "int") ||
          `int${"*".repeat(minDepth)}`;
        if (!base) {
          return missingDeclError(parsed.name, requiredBaseType);
        }
        const baseDepth = pointerDepth(base.type);
        if (!Number.isFinite(baseDepth) || baseDepth < minDepth) {
          return typeMismatchError(parsed.name, requiredBaseType);
        }
        const resolved = resolveUnaryLvalue(state, parsed.ops, parsed.name);
        if (resolved?.error === "empty") {
          return {
            error: `${resolved.label} doesn't have a value yet, so it can't be dereferenced.`,
            kind: "ub",
          };
        }
        if (resolved?.error === "unknown") {
          return {
            error: `${resolved.label} doesn't point to a known variable.`,
            kind: "ub",
          };
        }
        if (resolved?.error === "not_deref") {
          return {
            error: `${resolved.label} is not a valid dereference.`,
            kind: "compile",
          };
        }
        if (resolved?.error === "not_lvalue") {
          return { error: "That assignment is not valid here.", kind: "compile" };
        }
        const target = resolved?.target;
        if (!target) {
          return { error: "That assignment is not valid here.", kind: "compile" };
        }
        if (parsed.rhs.kind === "num") {
          const err = numericLiteralErrorForType(
            parsed.rhs.value,
            target.type,
          );
          if (err) return err;
        } else if (parsed.rhs.kind === "var") {
          const source = by[parsed.rhs.name];
          if (!source) {
            return {
              error: `You can't use ${parsed.rhs.name} before declaring it.`,
              kind: "compile",
            };
          }
          if (requireSourceValue && isEmptyVal(source.value ?? "")) {
            return {
              error: `${parsed.rhs.name} doesn't have a value yet.`,
              kind: "ub",
            };
          }
          const { base: targetBase, depth: targetDepth } = parseType(target.type);
          const { base: sourceBase, depth: sourceDepth } = parseType(source.type);
          const sameType = target.type === source.type;
          const isScalar =
            targetBase &&
            sourceBase &&
            targetBase === sourceBase &&
            targetDepth === 0 &&
            sourceDepth === 0;
          const isPtr = sameType && isPointerType(target.type);
          if (!isScalar && !isPtr) {
            return typeMismatchError(target.name, source.type);
          }
        } else if (parsed.rhs.kind === "ref") {
          const refBox = by[parsed.rhs.name];
          if (!refBox) {
            return {
              error: `You can't use ${parsed.rhs.name} before declaring it.`,
              kind: "compile",
            };
          }
          if (!isPointerType(target.type)) {
            const expected = expectedPointerTypeForRef(refBox.type) || "int*";
            return typeMismatchError(target.name, expected);
          }
          if (!isRefCompatible(target.type, refBox.type)) {
            const expected = expectedPointerTypeForRef(refBox.type);
            if (expected) return typeMismatchError(target.name, expected);
            return {
              error: "That assignment is not valid here.",
              kind: "compile",
            };
          }
        } else if (parsed.rhs.kind === "deref") {
          const ptr = by[parsed.rhs.name];
          if (!ptr) {
            return {
              error: `You can't use ${parsed.rhs.name} before declaring it.`,
              kind: "compile",
            };
          }
          const depth = parsed.rhs.depth || 1;
          const { base: ptrBase, depth: ptrDepth } = parseType(ptr.type);
          const { base: targetBase, depth: targetDepth } = parseType(target.type);
          if (!ptrBase || !targetBase) {
            return typeMismatchError(
              parsed.rhs.name,
              makePointerType(depth) || `int${"*".repeat(depth)}`,
            );
          }
          if (!Number.isFinite(ptrDepth) || ptrDepth < depth) {
            return typeMismatchError(
              parsed.rhs.name,
              makePointerType(depth, ptrBase) || `int${"*".repeat(depth)}`,
            );
          }
          const resultDepth = ptrDepth - depth;
          if (targetBase !== ptrBase || targetDepth !== resultDepth) {
            return typeMismatchError(
              target.name,
              makePointerType(resultDepth, ptrBase) ||
                `int${"*".repeat(resultDepth)}`,
            );
          }
          let current = ptr;
          const derefLabel = `${"*".repeat(depth)}${parsed.rhs.name}`;
          for (let i = 0; i < depth; i++) {
            if (!isPointerType(current.type)) {
              return typeMismatchError(
                parsed.rhs.name,
                makePointerType(depth, ptrBase) || `int${"*".repeat(depth)}`,
              );
            }
            if (!current.value || current.value === "empty") {
              return {
                error: `${derefLabel} doesn't have a value yet.`,
                kind: "ub",
              };
            }
            const next = state.find(
              (b) => b.address === String(current.value),
            );
            if (!next) {
              return {
                error: `${parsed.rhs.name} doesn't point to a known variable.`,
                kind: "ub",
              };
            }
            current = next;
          }
          if (requireSourceValue && isEmptyVal(current.value ?? "")) {
            return {
              error: `${derefLabel} doesn't have a value yet.`,
              kind: "ub",
            };
          }
        } else if (parsed.rhs.kind === "unary") {
          const err = validateUnaryRhs(
            state,
            target.type,
            target.name,
            parsed.rhs.ops,
            parsed.rhs.name,
          );
          if (err) return err;
        }
      } else if (parsed.kind === "assignUnaryRhs") {
        const by = Object.fromEntries(state.map((b) => [b.name, b]));
        const target = by[parsed.name];
        if (!target) {
          const typeLabel = "int";
          return missingDeclError(parsed.name, typeLabel);
        }
        const err = validateUnaryRhs(
          state,
          target.type,
          target.name,
          parsed.ops,
          parsed.src,
        );
        if (err) return err;
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
        const { base: ptrBase, depth: ptrDepth } = parseType(ptr.type);
        if (!ptrBase || !Number.isFinite(ptrDepth) || ptrDepth < depth) {
          return typeMismatchError(
            parsed.name,
            makePointerType(depth, ptrBase || "int") ||
              `int${"*".repeat(depth)}`,
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
          const { base: srcBase, depth: srcDepth } = parseType(
            by[parsed.src].type,
          );
          if (!srcBase || srcBase !== ptrBase) {
            return typeMismatchError(
              parsed.src,
              makePointerType(ptrDepth - depth, ptrBase) ||
                `int${"*".repeat(ptrDepth - depth)}`,
            );
          }
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
          const { base: refBase, depth: refDepth } = parseType(
            by[parsed.ref].type,
          );
          if (!refBase || refBase !== ptrBase) {
            return typeMismatchError(
              parsed.ref,
              makePointerType(ptrDepth - depth, ptrBase) ||
                `int${"*".repeat(ptrDepth - depth)}`,
            );
          }
          expectedPtrDepth = Number.isFinite(refDepth)
            ? depth + refDepth + 1
            : depth + 1;
        }
        if (ptrDepth !== expectedPtrDepth) {
          return typeMismatchError(
            parsed.name,
            makePointerType(expectedPtrDepth, ptrBase) ||
              `int${"*".repeat(expectedPtrDepth)}`,
          );
        }
        let current = ptr;
        for (let i = 0; i < depth; i++) {
          if (!isPointerType(current.type)) {
            return typeMismatchError(
              parsed.name,
              makePointerType(depth, ptrBase) || `int${"*".repeat(depth)}`,
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
        if (parsed.kind === "assignDeref") {
          const { target } = resolveDerefTarget(state, parsed.name, depth);
          if (target) {
            const err = numericLiteralErrorForType(parsed.value, target.type);
            if (err) return err;
          }
        }
      } else if (parsed.kind === "assignFromDeref") {
        const by = Object.fromEntries(state.map((b) => [b.name, b]));
        const target = by[parsed.name];
        if (!target) {
          return missingDeclError(parsed.name, "int");
        }
        const ptr = by[parsed.ptr];
        if (!ptr) {
          return {
            error: `You can't use ${parsed.ptr} before declaring it.`,
            kind: "compile",
          };
        }
        const depth = parsed.depth || 1;
        const { base: ptrBase, depth: ptrDepth } = parseType(ptr.type);
        const { base: targetBase, depth: targetDepth } = parseType(target.type);
        if (!ptrBase || !targetBase) {
          return typeMismatchError(
            parsed.ptr,
            makePointerType(depth) || `int${"*".repeat(depth)}`,
          );
        }
        if (!Number.isFinite(ptrDepth) || ptrDepth < depth) {
          return typeMismatchError(
            parsed.ptr,
            makePointerType(depth, ptrBase) || `int${"*".repeat(depth)}`,
          );
        }
        const resultDepth = ptrDepth - depth;
        if (targetBase !== ptrBase || targetDepth !== resultDepth) {
          return typeMismatchError(
            parsed.name,
            makePointerType(resultDepth, ptrBase) ||
              `int${"*".repeat(resultDepth)}`,
          );
        }
        let current = ptr;
        const derefLabel = `${"*".repeat(depth)}${parsed.ptr}`;
        for (let i = 0; i < depth; i++) {
          if (!isPointerType(current.type)) {
            return typeMismatchError(
              parsed.ptr,
              makePointerType(depth, ptrBase) || `int${"*".repeat(depth)}`,
            );
          }
          if (!current.value || current.value === "empty") {
            return {
              error: `${derefLabel} doesn't have a value yet.`,
              kind: "ub",
            };
          }
          const next = state.find((b) => b.address === String(current.value));
          if (!next) {
            return {
              error: `${parsed.ptr} doesn't point to a known variable.`,
              kind: "ub",
            };
          }
          current = next;
        }
        if (requireSourceValue && isEmptyVal(current.value ?? "")) {
          return {
            error: `${derefLabel} doesn't have a value yet.`,
            kind: "ub",
          };
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
                  const snippet = toStatementSnippet(
                    startLine,
                    startCol,
                    tok.line,
                  );
                  const text = `This statement spans multiple lines. In C, a line break acts like a space, so this statement is ${snippet}.`;
                  const html = `This statement spans multiple lines. In C, a line break acts like a space, so this statement is <code class="tok-line">${escapeHtml(snippet)}</code>.`;
                  info.set(tok.line, { text, html });
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
    const displayValue = emptyDisplay ? "" : normalizeZeroDisplay(value);
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
      const valueEl = node.querySelector(".value");
      valueEl.setAttribute("contenteditable", "true");
      disableAutoText(valueEl);
      if (allowTypeEdit) {
        const typeEl = node.querySelector(".type");
        typeEl.setAttribute("contenteditable", "true");
        typeEl.classList.add("editable");
        disableAutoText(typeEl);
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
              disableAutoText(textEl);
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
          disableAutoText(el);
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
        : normalizeZeroDisplay(valText);
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
    let boxes = [...ws.querySelectorAll(".vbox")];
    if (!boxes.length && ws.dataset.inline === "true") {
      boxes = [...document.querySelectorAll(`.vbox[data-workspace="${id}"]`)];
    }
    return boxes.map((v) => readBoxState(v));
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

  function setupInlineWorkspace(workspace, container, opts = {}) {
    if (!workspace || !container) return;
    const { useContents = false } = opts;
    workspace.dataset.inline = "true";
    const supportsContents =
      useContents &&
      typeof CSS !== "undefined" &&
      typeof CSS.supports === "function" &&
      CSS.supports("display", "contents");
    if (supportsContents) {
      workspace.classList.add("workspace-inline");
      workspace.style.display = "";
      return;
    }
    workspace.classList.remove("workspace-inline");
    workspace.style.display = "none";
    const id = workspace.id || "";
    const placeBox = (box) => {
      if (!box || !box.classList?.contains("vbox")) return;
      if (id) box.dataset.workspace = id;
      if (container.contains(workspace)) {
        container.insertBefore(box, workspace);
      } else {
        container.appendChild(box);
      }
    };
    workspace.querySelectorAll(".vbox").forEach((box) => placeBox(box));
    const observer = new MutationObserver((mutations) => {
      mutations.forEach((m) => {
        m.addedNodes.forEach((node) => {
          if (!(node instanceof Element)) return;
          if (node.classList?.contains("vbox")) {
            placeBox(node);
          } else {
            node.querySelectorAll?.(".vbox").forEach((box) => placeBox(box));
          }
        });
      });
    });
    observer.observe(workspace, { childList: true });
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
    disableAutoText(t);
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

  function isTextInputActive(el) {
    if (!el) return false;
    if (el.isContentEditable) return true;
    const tag = el.tagName;
    return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT";
  }

  document.addEventListener("keydown", (e) => {
    if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
    if (isTextInputActive(document.activeElement) || isTextInputActive(e.target))
      return;
    const selector =
      e.key === "ArrowLeft" ? 'button[id$="-prev"]' : 'button[id$="-next"]';
    const btn = document.querySelector(selector);
    if (!btn || btn.disabled) return;
    e.preventDefault();
    btn.click();
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
    if (document.body?.classList?.contains("no-scroll-hint")) return;
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

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", () => {
      applyAutoTextDefaults(document);
    });
  } else {
    applyAutoTextDefaults(document);
  }

  function initInstructionWatcher() {
    const seen = new Map();
    const normalizeInstruction = (text = "") =>
      String(text)
        .replace(/Run line\s+\d+/gi, "Run line #")
        .replace(/\s+/g, " ")
        .trim();
    document.querySelectorAll(".intro").forEach((el) => {
      const initialText = (el.textContent || "").trim();
      seen.set(el, {
        html: el.innerHTML,
        norm: normalizeInstruction(initialText),
      });
      const obs = new MutationObserver(() => {
        const prev = seen.get(el) || { html: "", norm: "" };
        const currHtml = el.innerHTML;
        if (currHtml === prev.html) return;
        const currText = (el.textContent || "").trim();
        const currNorm = normalizeInstruction(currText);
        seen.set(el, { html: currHtml, norm: currNorm });
        const text = (el.textContent || "").trim();
        if (text) {
          if (prev.norm === currNorm) return;
          requestAnimationFrame(() => {
            const rect = el.getBoundingClientRect();
            const offset = 24;
            const top = Math.max(0, rect.top + window.scrollY - offset);
            window.scrollTo({ top, behavior: "smooth" });
          });
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
    if (state == null) {
      const prefersCollapsed =
        window.matchMedia &&
        window.matchMedia("(max-width: 900px)").matches;
      if (prefersCollapsed) document.body.classList.add("sidebar-collapsed");
    }
  }

  applySidebarStateFromUrl();

  function initSidebarToggle() {
    const wrap = document.querySelector(".wrap");
    const nav = wrap?.querySelector("nav");
    if (!wrap || !nav) return;
    if (!nav.id) nav.id = "sidebar";
    let sidebarWrap = wrap.querySelector(".sidebar-wrap");
    if (!sidebarWrap) {
      sidebarWrap = document.createElement("div");
      sidebarWrap.className = "sidebar-wrap";
      wrap.insertBefore(sidebarWrap, nav);
      sidebarWrap.appendChild(nav);
    }
    let btn = document.querySelector(".sidebar-toggle");
    if (!btn) {
      btn = el(
        '<button type="button" class="sidebar-toggle"><span class="hamburger" aria-hidden="true"><span></span><span></span><span></span></span><span class="sr-only">Toggle sidebar</span></button>',
      );
      document.body.appendChild(btn);
    }
    btn.setAttribute("aria-controls", nav.id);
    const placeToggle = () => {
      if (btn.parentElement !== sidebarWrap) {
        sidebarWrap.insertBefore(btn, sidebarWrap.firstChild);
      }
    };
    const updateLabel = () => {
      const hidden = document.body.classList.contains("sidebar-collapsed");
      const label = hidden ? "Show sidebar" : "Hide sidebar";
      btn.classList.toggle("is-expanded", !hidden);
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
      placeToggle();
      updateUrl();
    });
    updateLabel();
    placeToggle();
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
    disableAutoText,
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
    setupInlineWorkspace,
    setHintContent,
    createHintController,
    createStepper,
    pulseNextButton,
    flashStatus,
    disableBoxEditing,
    removeBoxDeleteButtons,
  };
})(window);
