(function (MB) {
  const { $, vbox } = MB;

  const list = $("#reference-list");
  if (!list) return;

  function makeBox(opts, { hideName = false, hideAddr = false } = {}) {
    const node = vbox(opts);
    if (hideName) node.classList.add("no-name");
    if (hideAddr) node.classList.add("no-addr");
    annotateUnknowns(node);
    return node;
  }

  const UNKNOWN = "?";
  const OPTIONAL = "(?)";

  function annotateUnknowns(node) {
    if (!node) return;
    const entries = [
      { sel: ".addr", label: "address" },
      { sel: ".type", label: "type" },
      { sel: ".value", label: "value" },
      { sel: ".name-text", label: "name" },
    ];
    entries.forEach(({ sel, label }) => {
      node.querySelectorAll(sel).forEach((el) => {
        const text = (el.textContent || "").trim();
        if (text === UNKNOWN) {
          el.title = `This ${label} is unknown, but it will not be absent.`;
        } else if (text === OPTIONAL) {
          el.title = `This ${label} may or may not exist.`;
        }
      });
    });
  }

  function labeledBox(label, box) {
    const wrap = document.createElement("div");
    wrap.className = "ref-box";
    const tag = document.createElement("div");
    tag.className = "ref-box-label";
    tag.textContent = label;
    wrap.appendChild(tag);
    wrap.appendChild(box);
    return wrap;
  }

  function arrow(label, symbol = "→") {
    const wrap = document.createElement("div");
    wrap.className = "ref-arrow";
    const text = document.createElement("div");
    text.className = "ref-arrow-text";
    text.textContent = symbol;
    wrap.appendChild(text);
    if (label) {
      const lbl = document.createElement("div");
      lbl.className = "ref-arrow-label";
      lbl.textContent = label;
      wrap.appendChild(lbl);
    }
    return wrap;
  }

  function op(symbol) {
    const wrap = document.createElement("div");
    wrap.className = "ref-op";
    wrap.textContent = symbol;
    return wrap;
  }

  function expr(text) {
    const wrap = document.createElement("div");
    wrap.className = "ref-expr";
    wrap.textContent = text;
    return wrap;
  }

  function label(text) {
    const wrap = document.createElement("div");
    wrap.className = "ref-label";
    wrap.textContent = text;
    return wrap;
  }

  function note(text) {
    const wrap = document.createElement("div");
    wrap.className = "ref-note-inline";
    wrap.textContent = text;
    return wrap;
  }

  function inlineRow() {
    const wrap = document.createElement("div");
    wrap.className = "ref-inline";
    return wrap;
  }
  function card({ title, syntax, text, note, buildVisual }) {
    const card = document.createElement("div");
    card.className = "ref-card";
    const row = document.createElement("div");
    row.className = "ref-row";

    const copy = document.createElement("div");
    copy.className = "ref-copy";

    const heading = document.createElement("h2");
    heading.textContent = title;
    copy.appendChild(heading);

    if (syntax) {
      const code = document.createElement("div");
      code.className = "ref-code";
      code.textContent = syntax;
      copy.appendChild(code);
    }

    if (text) {
      const body = document.createElement("p");
      body.className = "ref-text";
      body.textContent = text;
      copy.appendChild(body);
    }

    if (note) {
      const noteEl = document.createElement("p");
      noteEl.className = "ref-note";
      noteEl.textContent = note;
      copy.appendChild(noteEl);
    }

    const visual = document.createElement("div");
    visual.className = "ref-visual";
    buildVisual(visual);

    row.appendChild(copy);
    row.appendChild(visual);
    card.appendChild(row);
    list.appendChild(card);
  }

  function addSection(title) {
    const section = document.createElement("div");
    section.className = "ref-section";
    section.textContent = title;
    list.appendChild(section);
  }

  addSection("Statements");

  card({
    title: "Declaration",
    syntax: "T N;",
    text: "Creates a new variable box named N of type T with an empty value.",
    buildVisual: (visual) => {
      visual.appendChild(expr("T N;"));
      visual.appendChild(arrow("creates"));
      visual.appendChild(
        makeBox({ addr: UNKNOWN, type: "T", value: "empty", name: "N" }),
      );
    },
  });

  card({
    title: "Assignment",
    syntax: "N = V;",
    text: "Stores V into the existing box named N.",
    buildVisual: (visual) => {
      const before = makeBox({
        addr: "A",
        type: "T",
        value: OPTIONAL,
        name: "N",
      });
      const boxValue = makeBox({
        addr: OPTIONAL,
        type: "T",
        value: "V",
        name: OPTIONAL,
      });
      const oldBox = makeBox({
        addr: "A",
        type: "T",
        value: OPTIONAL,
        name: "N",
      });
      const updated = makeBox({
        addr: "A",
        type: "T",
        value: "V",
        name: "N",
      });
      visual.appendChild(before);
      visual.appendChild(op("="));
      visual.appendChild(boxValue);
      visual.appendChild(op(";"));
      visual.appendChild(document.createElement("div")).className = "ref-break";
      visual.appendChild(arrow("changes this box"));
      visual.appendChild(oldBox);
      visual.appendChild(arrow("to this"));
      visual.appendChild(updated);
    },
  });

  card({
    title: "Initialization",
    syntax: "T N = V;",
    text: "Creates N and stores V in it immediately.",
    note: "Equivalent to: T N; then N = V;",
    buildVisual: (visual) => {
      visual.appendChild(expr("T N = V;"));
      visual.appendChild(arrow("creates"));
      visual.appendChild(
        makeBox({ addr: UNKNOWN, type: "T", value: "V", name: "N" }),
      );
    },
  });

  addSection("Expressions");

  card({
    title: "Integer",
    syntax: "123",
    text: "Evaluates to an integer rvalue.",
    buildVisual: (visual) => {
      const box = makeBox(
        { addr: "", type: "int", value: "123", name: UNKNOWN },
        { hideAddr: true, hideName: true },
      );
      visual.appendChild(note("(for example)"));
      const row = inlineRow();
      row.appendChild(expr("123"));
      row.appendChild(arrow("evaluates to"));
      row.appendChild(box);
      visual.appendChild(row);
    },
  });

  card({
    title: "Variable Name",
    syntax: "N",
    text: "Evaluates to the box named N.",
    buildVisual: (visual) => {
      const nBox = makeBox({
        addr: UNKNOWN,
        type: UNKNOWN,
        value: OPTIONAL,
        name: "N",
      });
      visual.appendChild(expr("N"));
      visual.appendChild(arrow("evaluates to"));
      visual.appendChild(nBox);
    },
  });

  card({
    title: "Address Of",
    syntax: "&N",
    text: "Evaluates to a pointer value holding N's address.",
    buildVisual: (visual) => {
      const nBox = makeBox({
        addr: "A",
        type: "T",
        value: OPTIONAL,
        name: UNKNOWN,
      });
      const pBox = makeBox(
        { addr: "", type: "T*", value: "A", name: UNKNOWN },
        { hideAddr: true, hideName: true },
      );
      visual.appendChild(op("&"));
      visual.appendChild(nBox);
      visual.appendChild(arrow("evaluates to"));
      visual.appendChild(pBox);
    },
  });

  card({
    title: "Dereference",
    syntax: "*P",
    text: "Evaluates to the box at the address stored in P.",
    buildVisual: (visual) => {
      const ptr = makeBox(
        { addr: OPTIONAL, type: "T*", value: "A", name: OPTIONAL },
      );
      const target = makeBox(
        { addr: "A", type: "T", value: OPTIONAL, name: UNKNOWN },
      );
      visual.appendChild(op("*"));
      visual.appendChild(ptr);
      visual.appendChild(arrow("evaluates to"));
      visual.appendChild(target);
    },
  });

  addSection("Operators");

  card({
    title: "Parentheses",
    syntax: "(E)",
    text: "Evaluates E first. Use parentheses to override precedence.",
    buildVisual: (visual) => {
      const before = makeBox(
        { addr: OPTIONAL, type: "int", value: "E", name: OPTIONAL },
      );
      const after = makeBox(
        { addr: OPTIONAL, type: "int", value: "E", name: OPTIONAL },
      );
      visual.appendChild(op("("));
      visual.appendChild(before);
      visual.appendChild(op(")"));
      visual.appendChild(arrow("same value as"));
      visual.appendChild(after);
      visual.appendChild(document.createElement("div")).className = "ref-break";
      visual.appendChild(expr("(1 + 2) * 3"));
      visual.appendChild(arrow("evaluates as"));
      visual.appendChild(expr("3 * 3"));
    },
  });

  card({
    title: "Unary Plus / Minus",
    syntax: "+E, -E",
    text: "Evaluates E, then keeps it (+) or negates it (-).",
    note: "Unary operators have higher precedence than * or /.",
    buildVisual: (visual) => {
      const lhs = makeBox(
        { addr: OPTIONAL, type: "int", value: "E", name: OPTIONAL },
      );
      const rhs = makeBox(
        { addr: "", type: "int", value: "R", name: UNKNOWN },
        { hideAddr: true, hideName: true },
      );
      visual.appendChild(op("+/-"));
      visual.appendChild(lhs);
      visual.appendChild(arrow("evaluates to"));
      visual.appendChild(rhs);
      visual.appendChild(document.createElement("div")).className = "ref-break";
      const exLeft = makeBox(
        { addr: "", type: "int", value: "5", name: UNKNOWN },
        { hideAddr: true, hideName: true },
      );
      const exRight = makeBox(
        { addr: "", type: "int", value: "-5", name: UNKNOWN },
        { hideAddr: true, hideName: true },
      );
      visual.appendChild(op("-"));
      visual.appendChild(exLeft);
      visual.appendChild(arrow("evaluates to"));
      visual.appendChild(exRight);
    },
  });

  card({
    title: "Multiplication",
    syntax: "A * B",
    text: "Evaluates to the product of A and B.",
    note: "Binary * has higher precedence than +, -, and ==.",
    buildVisual: (visual) => {
      const left = makeBox(
        { addr: OPTIONAL, type: "int", value: "A", name: OPTIONAL },
      );
      const right = makeBox(
        { addr: OPTIONAL, type: "int", value: "B", name: OPTIONAL },
      );
      const result = makeBox(
        { addr: "", type: "int", value: "R", name: UNKNOWN },
        { hideAddr: true, hideName: true },
      );
      visual.appendChild(left);
      visual.appendChild(op("*"));
      visual.appendChild(right);
      visual.appendChild(arrow("evaluates to"));
      visual.appendChild(result);
      visual.appendChild(document.createElement("div")).className = "ref-break";
      const exLeft = makeBox(
        { addr: "", type: "int", value: "2", name: UNKNOWN },
        { hideAddr: true, hideName: true },
      );
      const exRight = makeBox(
        { addr: "", type: "int", value: "3", name: UNKNOWN },
        { hideAddr: true, hideName: true },
      );
      const exResult = makeBox(
        { addr: "", type: "int", value: "6", name: UNKNOWN },
        { hideAddr: true, hideName: true },
      );
      visual.appendChild(exLeft);
      visual.appendChild(op("*"));
      visual.appendChild(exRight);
      visual.appendChild(arrow("evaluates to"));
      visual.appendChild(exResult);
    },
  });

  card({
    title: "Division",
    syntax: "A / B",
    text: "Evaluates to the integer quotient of A divided by B.",
    note: "Binary / has the same precedence as *.",
    buildVisual: (visual) => {
      const left = makeBox(
        { addr: OPTIONAL, type: "int", value: "A", name: OPTIONAL },
      );
      const right = makeBox(
        { addr: OPTIONAL, type: "int", value: "B", name: OPTIONAL },
      );
      const result = makeBox(
        { addr: "", type: "int", value: "R", name: UNKNOWN },
        { hideAddr: true, hideName: true },
      );
      visual.appendChild(left);
      visual.appendChild(op("/"));
      visual.appendChild(right);
      visual.appendChild(arrow("evaluates to"));
      visual.appendChild(result);
      visual.appendChild(document.createElement("div")).className = "ref-break";
      const exLeft = makeBox(
        { addr: "", type: "int", value: "7", name: UNKNOWN },
        { hideAddr: true, hideName: true },
      );
      const exRight = makeBox(
        { addr: "", type: "int", value: "2", name: UNKNOWN },
        { hideAddr: true, hideName: true },
      );
      const exResult = makeBox(
        { addr: "", type: "int", value: "3", name: UNKNOWN },
        { hideAddr: true, hideName: true },
      );
      visual.appendChild(exLeft);
      visual.appendChild(op("/"));
      visual.appendChild(exRight);
      visual.appendChild(arrow("evaluates to"));
      visual.appendChild(exResult);
    },
  });

  card({
    title: "Addition / Subtraction",
    syntax: "A + B, A - B",
    text: "Evaluates to the sum or difference of A and B.",
    note: "Binary + and - have lower precedence than * and /.",
    buildVisual: (visual) => {
      const left = makeBox(
        { addr: OPTIONAL, type: "int", value: "A", name: OPTIONAL },
      );
      const right = makeBox(
        { addr: OPTIONAL, type: "int", value: "B", name: OPTIONAL },
      );
      const result = makeBox(
        { addr: "", type: "int", value: "R", name: UNKNOWN },
        { hideAddr: true, hideName: true },
      );
      visual.appendChild(left);
      visual.appendChild(op("+/-"));
      visual.appendChild(right);
      visual.appendChild(arrow("evaluates to"));
      visual.appendChild(result);
      visual.appendChild(document.createElement("div")).className = "ref-break";
      const exLeft = makeBox(
        { addr: "", type: "int", value: "1", name: UNKNOWN },
        { hideAddr: true, hideName: true },
      );
      const exRight = makeBox(
        { addr: "", type: "int", value: "2", name: UNKNOWN },
        { hideAddr: true, hideName: true },
      );
      const exResult = makeBox(
        { addr: "", type: "int", value: "3", name: UNKNOWN },
        { hideAddr: true, hideName: true },
      );
      visual.appendChild(exLeft);
      visual.appendChild(op("+"));
      visual.appendChild(exRight);
      visual.appendChild(arrow("evaluates to"));
      visual.appendChild(exResult);
    },
  });

  card({
    title: "Equality",
    syntax: "A == B",
    text: "Evaluates to 1 if A and B are equal, otherwise 0.",
    note: "== has lower precedence than +, -, * and /.",
    buildVisual: (visual) => {
      const left = makeBox(
        { addr: OPTIONAL, type: "int", value: "A", name: OPTIONAL },
      );
      const right = makeBox(
        { addr: OPTIONAL, type: "int", value: "B", name: OPTIONAL },
      );
      const result = makeBox(
        { addr: "", type: "int", value: "R", name: UNKNOWN },
        { hideAddr: true, hideName: true },
      );
      visual.appendChild(left);
      visual.appendChild(op("=="));
      visual.appendChild(right);
      visual.appendChild(arrow("evaluates to"));
      visual.appendChild(result);
      visual.appendChild(document.createElement("div")).className = "ref-break";
      const exLeft = makeBox(
        { addr: "", type: "int", value: "3", name: UNKNOWN },
        { hideAddr: true, hideName: true },
      );
      const exRight = makeBox(
        { addr: "", type: "int", value: "4", name: UNKNOWN },
        { hideAddr: true, hideName: true },
      );
      const exResult = makeBox(
        { addr: "", type: "int", value: "0", name: UNKNOWN },
        { hideAddr: true, hideName: true },
      );
      visual.appendChild(exLeft);
      visual.appendChild(op("=="));
      visual.appendChild(exRight);
      visual.appendChild(arrow("evaluates to"));
      visual.appendChild(exResult);
    },
  });

  card({
    title: "Order of Operations",
    syntax: "Highest to lowest",
    text: "Parentheses, unary +/-, *, /, binary +/-, ==.",
    note: "Operators at the same level evaluate left to right.",
    buildVisual: (visual) => {
      visual.appendChild(expr("1 + 2 * 3 == 7"));
      visual.appendChild(arrow("evaluates as"));
      visual.appendChild(expr("1 + 6 == 7"));
    },
  });
})(window.MB || {});
