import { createProgramTemplate } from "./shared-program-template.js";

createProgramTemplate({
  initialInstructions:
    "With $c{while}, we can start writing some interesting programs. This program determines the first Fibonacci number greater than 100.\nThere aren't any problem steps for this one; you can run through it for your own curiosity.",
  steps: [
    { code: "int last = 0;\n" },
    { code: "int curr = 1;\n" },
    { code: "while (curr <= 100) {\n" },
    { code: "  curr = last + curr;\n" },
    { code: "  last = curr - last;\n" },
    { code: "}\n" },
  ],
  workspace: {},
  next: "sandbox.html?finished=1",
  isLast: true,
});
