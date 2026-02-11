import { createProgramTemplate } from "./shared-program-template.js";

createProgramTemplate({
  initialInstructions:
    "A $c{while} loop will run repeatedly until its expression is zero.",
  steps: [
    { code: "int i = 2;\n" },
    { code: "while (i) {\n" },
    { code: "  int j = i - 1;\n" },
    { code: "  i = j;\n" },
    { code: "}\n" },
    { code: "int x = 3;\n" },
    {
      code: "while (x >= 0) {\n",
      editable: true,
      instructions: "Select the line boundary where execution will resume.",
    },
    { code: "  x = x - i - 2;\n", editable: true },
    {
      code: "}\n",
      editable: true,
    },
  ],
  next: "sandbox.html?finished=1",
  isLast: true,
  workspace: { allowVariableCreation: true, allowVariableDeletion: true },
});
