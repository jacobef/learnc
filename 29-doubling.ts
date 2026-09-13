import { createProgramTemplate } from "./shared-program-template.js";

createProgramTemplate({
  initialInstructions:
    "This program determines the first power of two (1, 2, 4, 8, 16, ...) that is at least 25.\nThere aren't any problem steps for this one; it's just meant as an example. Soon, you will be writing your own programs using $c{while}!",
  steps: [
    { code: "int x = 1;\n" },
    { code: "while (x < 25) {\n" },
    { code: "  x = x * 2;\n" },
    { code: "}\n" },
  ],
  workspace: {},
  next: "30-square-root.html",
});
