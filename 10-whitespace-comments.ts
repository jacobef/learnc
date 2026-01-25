import { createProgramTemplate } from "./shared-program-template.js";

createProgramTemplate({
  initialInstructions: "Click $runLineButton to continue.",
  steps: [
    {
      code: "int a; // mary had a\n",
      editable: false,
      instructions:
        "Anything after $c{//} on a line is ignored. This is called a line comment.",
    },
    {
      code: "// little lamb\n",
      editable: false,
      instructions: "Comments can appear on their own lines as well.",
    },
    {
      code: "int b; int c;\n",
      editable: false,
      instructions: "Multiple statements can appear on one line.",
    },
    {
      code: "int d; // int e;\n",
      editable: true,
      hints: (ctx) => {
        if (ctx.boxNamed("e")) {
          return "$c{// int e;} is a comment.";
        }
        return ctx.basicHint;
      },
    },
    {
      code: "int f\n= 5\n;\n",
      editable: false,
      instructions: "A statement can be split across multiple lines.",
    },
    {
      code: "/* whose fleece\nwas white as snow */\n",
      editable: false,
      instructions:
        "A comment can appear on multiple lines, or within a line, beginning with $c{/*} and ending with $c{*/}. This is called a block comment.",
    },
    {
      code: "int g /* hi */ = 3;\n",
      editable: false,
      instructions:
        "A comment can appear on multiple lines, or within a line, beginning with $c{/*} and ending with $c{*/}. This is called a block comment.",
    },
    {
      code: "int\nh // = 9\n/* = 10\n= 11\n*/ = 12\n// = 13\n;\n",
      editable: true,
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "h")) {
          const h = ctx.boxNamed("h")!;
          if (h.value === "9") {
            return "$c{// = 9;} is a comment.";
          }
          if (h.value === "10" || h.value === "11") {
            return "$c{/* = 10;\n= 11;\n*/} is a comment.";
          }
          if (h.value === "13") {
            return "$c{// = 13;} is a comment.";
          }
          if (h.value === "") {
            return "$n{h}'s value shouldn't be empty; one of the number assignments isn't part of a comment.";
          }
          return `I'm not sure where you're getting $v{${h.value}} from.`;
        }
        return ctx.basicHint;
      },
    },
  ],
  next: "11-integer-arithmetic.html",
  workspace: { allowVariableCreation: true },
});
