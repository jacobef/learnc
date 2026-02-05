import { createProgramTemplate } from "./shared-program-template.js";

createProgramTemplate({
  initialInstructions:
    "Here's our first program that performs an actually meaningful computation. It calculates the absolute value of $n{x}. Here, $n{x} happens to be $v{-91}, but it would work for any value.",
  steps: [
    {
      code: "int x = -91;\n",
      editable: false,
    },
    {
      code: "int abs_x;\n",
      editable: false,
    },
    {
      code: "if (x >= 0) {\n",
      editable: true,
      instructions: "Select the line boundary where execution will resume.",
    },
    {
      code: "  abs_x = x;\n",
      editable: false,
    },
    {
      code: "} else {\n",
      editable: false,
    },
    {
      code: "  abs_x = -x;\n",
      editable: true,
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "abs_x")) {
          return "What is -(-91)?";
        }
        return ctx.basicHint;
      },
    },
    {
      code: "}\n",
      editable: true,
      hints: (ctx) => ctx.basicHint,
    },
    {
      code: "// abs_x now holds the absolute value of x\n",
      editable: false,
      instructions:
        "If you want a challenge, paste this level's code into the Sandbox, and rewrite it to not use $c{else}, but still work the same (for all possible values of $n{x}). Extra hard: make it not use $c{if} either.",
    },
  ],
  workspace: { allowVariableCreation: true, allowVariableDeletion: true },
  next: "sandbox.html?finished=1",
  isLast: true,
});
