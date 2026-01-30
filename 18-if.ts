import { createProgramTemplate } from "./shared-program-template.js";

createProgramTemplate({
  initialInstructions: "Click $runLineButton to continue.",
  steps: [
    {
      code: "int tree = 5;\n",
      editable: false,
      instructions:
        "The block after $c{if (3-3)} will run if $c{3-3} is non-zero. Otherwise, it will be skipped.",
    },
    {
      code: "if (3-3) {\n",
      editable: false,
      instructions: "hmm",
    },
    {
      code: "  tree = 10;\n}\n",
      editable: false,
    },
    {
      code: "double olive = 1.0;\n",
      editable: false,
    },
    {
      code: "if (olive) {\n",
      editable: false,
      instructions: "$v{1.0} is non-zero, so the block is not skipped.",
    },
    {
      code: "  olive = 5.0;\n",
      editable: false,
    },
    {
      code: "  int leaf = 0;\n",
      editable: false,
    },
    {
      code: "}\n",
      editable: true,
      hints: (ctx) => {
        if (ctx.boxNamed("leaf")) {
          return "$n{leaf} was created in this block, so it should be removed.";
        }
        return ctx.basicHint;
      },
    },
    {
      code: "if (tree == 10) {\n",
      editable: true,
      instructions:
        "In the Code pane, select the line boundary where execution will resume. If the $c{if} block will be run, select the boundary between lines 10 and 11. If not, select the boundary between lines 12 and 13. Then press $checkButton.",
    },
    {
      code: "  olive = -olive;\n",
      editable: false,
    },
    {
      code: "}\n",
      editable: false,
    },
    {
      code: "if (5*0) {\n",
      editable: true,
      instructions: "Select the line boundary where execution will resume.",
    },
    {
      code: "  double twig = 0.1;\n",
      editable: false,
    },
    {
      code: "}\n",
      editable: false,
    },
    {
      code: "if (0 == -0) {\n",
      editable: true,
    },
    {
      code: "  int stem = 1;\n",
      editable: false,
    },
    {
      code: "  if (tree >= stem + 5) {\n",
      editable: true,
    },
    {
      code: "    double root;\n",
      editable: false,
    },
    {
      code: "  }\n",
      editable: false,
    },
    {
      code: "}\n",
      editable: true,
      hints: (ctx) => {
        if (ctx.boxNamed("stem")) {
          return "$n{stem} was created in this block, so it should be removed.";
        }
        return ctx.basicHint;
      },
    },
  ],
  next: "sandbox.html?finished=1",
  isLast: true,
  workspace: { allowVariableCreation: true, allowVariableDeletion: true },
});
