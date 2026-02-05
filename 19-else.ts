import { createProgramTemplate } from "./shared-program-template.js";

createProgramTemplate({
  initialInstructions:
    "When an $c{if} block is run, its $c{else} block is not. When an $c{if} block is not run, its $c{else} block is.",
  steps: [
    {
      code: "if (1) {\n",
      editable: false,
    },
    {
      code: "  int iron = 1;\n",
      editable: false,
    },
    {
      code: "} else {\n",
      editable: false,
    },
    {
      code: "  int iron = 2;\n",
      editable: false,
    },
    {
      code: "}\n",
      editable: false,
    },
    {
      code: "if (0) {\n",
      editable: false,
    },
    {
      code: "  int iron = 1;\n",
      editable: false,
    },
    {
      code: "} else {\n",
      editable: false,
    },
    {
      code: "  int iron = 2;\n",
      editable: false,
    },
    {
      code: "}\n",
      editable: false,
    },
    {
      code: "if (1 - 1 * -1) {\n",
      editable: true,
      instructions: "Select the line boundary where execution will resume.",
    },
    {
      code: "  double copper = 1 - 1 * -1;\n",
      editable: true,
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "copper")) {
          const copper = ctx.boxNamed("copper")!;
          if (copper.value === "2") {
            return "$n{copper} is a $t{double}, so $v{2} should get converted to $v{2.0}.";
          }
          if (copper.value === "0" || copper.value === "0.0") {
            return "You're probably parsing it as $c{(1 - 1) * -1}, but it should be parsed as $c{1 - (1 * -1)}.";
          }
          if (copper.value === "") {
            return "$n{copper}'s value shouldn't be empty.";
          }
          return `I'm not sure where you're getting $v{${copper.value}} from.`;
        }
        return ctx.basicHint;
      },
    },
    {
      code: "} else {\n",
      editable: false,
      hints: (ctx) => ctx.basicHint,
    },
    {
      code: "  int copper = 1 - 1 * -1;\n}\n",
      editable: false,
    },
    {
      code: "int gold;\n",
      editable: false,
    },
    {
      code: "if (1 * -1 - 1) {\n  int tin = 0;\n  gold = 0;\n} else {\n  gold = 1;\n  double silver = 1;\n}\n",
      editable: true,
      instructions:
        "What should the program state look like after this entire if-else statement is run?",
      hints: (ctx) => {
        if (ctx.boxNamed("tin")) {
          return "$n{tin} is declared inside the $c{if} block, so it would be removed at the $c{\\}} on line 23.";
        }
        if (ctx.boxNamed("silver")) {
          return "$n{silver} is declared in the $c{else} block, but the condition is non-zero, so the $c{else} block is skipped and $n{silver} should not exist. Also, even if the $c{else} block wasn't skipped, $n{silver}'s would have been removed with the $c{\\}} on line 27.";
        }
        if (!ctx.boxNamed("gold")) {
          return "$n{gold} was declared before this if-else statement, so it should still exist afterwards.";
        }
        if (ctx.basicHintTopicIs("value", "gold")) {
          const gold = ctx.boxNamed("gold")!;
          if (gold.value === "") {
            return "When a block ends, it only removes variables that were created in that block; it doesn't undo the other things that happened in that block, such as assigning to $n{gold}. $n{gold} should not be empty.";
          }
          if (gold.value === "1") {
            return "$c{1 * -1 - 1} is -2, which is non-zero, so the $c{if} block is ran, not the $c{else} block. That means $n{gold} should end up as $v{0}, not $v{1}.";
          }
          return `I'm not sure where you're getting $v{${gold.value}} from.`;
        }
        return ctx.basicHint;
      },
    },
  ],
  next: "20-abs.html",
  workspace: { allowVariableCreation: true, allowVariableDeletion: true },
});
