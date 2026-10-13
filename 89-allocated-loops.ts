import { createProgramTemplate } from "./shared-program-template.js";

createProgramTemplate({
  initialInstructions:
    "Since $c{malloc} is called while the program runs, an array from $c{malloc} can have any size the program computes, unlike an array variable, whose size is usually written in the code. (C also has variable-length arrays, array variables whose length is computed while the program runs. But compilers don't have to support them, and a big one can crash the program, so many programmers avoid them.)\nClick $runLineButton to continue.",
  steps: [
    { code: "#include <stdlib.h>\n" },
    { code: "int n = 4;\n" },
    { code: "int* squares = malloc(n * sizeof(int));\n" },
    { code: "for (int i = 0; i < n; i++) {\n" },
    {
      code: "  squares[i] = i * i;\n",
      editable: [false, true, false, true],
      hints: (ctx) => ctx.basicHint,
    },
    { code: "}\n" },
    { code: "int total = 0;\n" },
    {
      code: "for (int i = 0; i < n; i++) {\n  total += squares[i];\n}\n",
      editable: true,
      instructions: "What should the program state look like after this whole loop runs?",
      hints: (ctx) =>
        ctx.basicHintTopicIs("value", "total") ? "$n{total} adds up all of the elements: $c{0 + 1 + 4 + 9}." : ctx.basicHint,
    },
  ],
  next: "90-largest.html",
});
