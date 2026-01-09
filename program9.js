{
  const { createProgramTemplate } = window.MB;

  createProgramTemplate({
    lines: [
      "int a; // mary had a",
      "// little lamb",
      "int b; int c;",
      "int d; // int e;",
      "int f",
      "= 5",
      ";",
      "/* whose fleece",
      "was white as snow */",
      "int g /* hi */ = 3;",
      "int",
      "h // = 9;",
      "/* = 10;",
      "= 11;",
      "*/ = 12;",
      "// = 13;",
    ],
    editableSteps: [4, 11],
    stepperFallback: true,
    runGroups: [{ start: 11, end: 16 }],
    next: "program10.html",
    workspace: { allowAddAndDelete: true },
    instructions: (ctx) => {
      if (ctx.boundary === 1) {
        return "Anything after $c{//} on a line is ignored. This is called a line comment.";
      }
      if (ctx.boundary === 2) {
        return "Comments can appear on their own lines as well.";
      }
      if (ctx.boundary === 3) {
        return "Multiple statements can appear on one line.";
      }
      if (ctx.boundary === 7) {
        return "A statement can be split across multiple lines.";
      }
      if (
        ctx.boundary === 8 ||
        ctx.boundary === 9 ||
        ctx.boundary === 10 ||
        ctx.boundary === 13 ||
        ctx.boundary === 14
      ) {
        return "A comment can appear on multiple lines, or within a line, beginning with $c{/*} and ending with $c{*/}. This is called a block comment.";
      }
      return null;
    },
    hints: {
      4: (ctx) => {
        const by = ctx.byName || {};
        if (!by.d) {
          return "Add the $n{d} variable.";
        }
        if (by.e) {
          return "$c{// int e;} is a comment.";
        }
        if (by.d.type !== "int") {
          return "$n{d}'s type should be $t{int}.";
        }
        if (by.d.value !== "") {
          return "$n{d}'s value should be empty.";
        }
        return null;
      },
      11: (ctx) => {
        const by = ctx.byName || {};
        if (!by.h) {
          return "Add the $n{h} variable.";
        }
        if (by.h.type !== "int") {
          return "$n{h}'s type should be $t{int}.";
        }
        if (by.h.value === "9") {
          return "$c{// = 9;} is a comment.";
        } else if (by.h.value === "10" || by.h.value === "11") {
          return "$c{/* = 10;\n= 11;\n*/} is a comment.";
        } else if (by.h.value === "13") {
          return "$c{// = 13;} is a comment.";
        } else if (by.h.value === "") {
          return "$n{h}'s value shouldn't be empty; one of these numbers is not inside a comment.";
        } else if (by.h.value !== "12") {
          return `I'm not sure where you're getting $v{${by.h.value}} from.`;
        }
        return null;
      },
    },
  });
}
