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
})(window.MB || {});
