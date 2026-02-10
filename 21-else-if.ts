import { createProgramTemplate } from "./shared-program-template.js";

createProgramTemplate({
  initialInstructions:
    "The first $c{if} / $c{else if} block with a nonzero expression will run. If all the expressions are zero, the $c{else} block will run.\nIf you know Python, $c{else if} is C's equivalent to Python's $c{elif}.",
  steps: [
    { code: "int x = 78;\n" },
    { code: "if (x < 60) {\n" },
    { code: "  x = 1;\n" },
    { code: "} else if (x < 70) {\n" },
    { code: "  x = 2;\n" },
    { code: "} else if (x < 80) {\n" },
    { code: "  x = 3;\n" },
    { code: "} else if (x < 90) {\n" },
    { code: "  x = 4;\n" },
    { code: "} else {\n" },
    { code: "  x = 5;\n" },
    { code: "}\n" },
    { code: "int y = 0;\n" },
    {
      code: `if (y) {
  x = x + y;
} else if (x == y) {
  x = x - y;
} else {
  x = x * y;
}
`,
      editable: true,
      instructions:
        "What should the program state look like after this entire statement is run?",
    },
  ],
  next: "sandbox.html?finished=1",
  isLast: true,
  workspace: { allowVariableCreation: true, allowVariableDeletion: true },
});
