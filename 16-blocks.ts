import { createProgramTemplate } from "./shared-program-template.js";

createProgramTemplate({
  initialInstructions:
    "For once, I've made the whitespace here helpful rather than misleading.\nNo other instructions for this one. Good luck!",
  steps: [
    { code: "int brick = 10;\n" },
    { code: "{\n" },
    { code: "  int stone;\n" },
    { code: "  double glass = 3.5;\n" },
    { code: "  brick = 5;\n" },
    { code: "}\n" },
    { code: "{\n" },
    {
      code: "  int steel = 9;\n",
      editable: true,
      hints: (ctx) => ctx.basicHint,
    },
    {
      code: "}\n",
      editable: true,
      hints: (ctx) => {
        if (ctx.boxNamed("steel")) {
          return "This brace ends the block that created $n{steel}, so $n{steel} should be removed.";
        }
        if (!ctx.boxNamed("brick")) {
          return "$n{brick} was created outside this block, so it shouldn't be removed.";
        }
        return ctx.basicHint;
      },
    },
    { code: "{\n" },
    {
      code: "  double wood = brick == 10;\n",
      editable: true,
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "wood")) {
          const wood = ctx.boxNamed("wood")!;
          if (wood.value === "1" || wood.value === "1.0") {
            return "$n{brick}'s value is $v{5}, so $c{brick == 10} is false.";
          }
          if (wood.value === "0") {
            return "$c{brick == 10} does evaluate to $v{0}, but remember that we're assigning it to a $t{double}.";
          }
          if (wood.value === "") {
            return "Evaluate $c{brick == 10}, then convert the result to $t{double}.";
          }
          return `I'm not sure where you're getting $v{${wood.value}} from.`;
        }
        return ctx.basicHint;
      },
    },
    { code: "  int* copper;\n" },
    { code: "  {\n" },
    {
      code: "    int straw = 9.75;\n",
      editable: true,
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "straw")) {
          return "Assigning to an $t{int} should drop the decimal part.";
        }
        return ctx.basicHint;
      },
    },
    { code: "  }\n" },
    { code: "  {\n" },
    { code: "    int plastic;\n" },
    {
      code: "    wood = brick;\n",
      editable: true,
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "wood")) {
          const wood = ctx.boxNamed("wood")!;
          if (wood.value === "5") {
            return "Remember that $n{wood} is a $t{double}.";
          }
          return "Set $n{wood}'s value to $n{brick}'s value, converted to $t{double}.";
        }
        return ctx.basicHint;
      },
    },
    {
      code: "  }\n",
      editable: true,
      hints: (ctx) => {
        if (ctx.boxNamed("plastic")) {
          return "This brace ends the block that created $n{plastic}, so $n{plastic} should be removed.";
        }
        if (
          !ctx.boxNamed("copper") ||
          !ctx.boxNamed("wood") ||
          !ctx.boxNamed("brick")
        ) {
          return "$n{copper}, $n{wood}, and $n{brick} were created outside of this block, so none of them should be removed yet.";
        }
        if (ctx.basicHintTopicIs("value", "wood")) {
          return "When a block ends, all variables created in that block are removed, but it doesn't undo the other effects of that block. Specifically, it shouldn't undo $c{wood = brick;}.";
        }
        return ctx.basicHint;
      },
    },
    {
      code: "}\n",
      editable: true,
      hints: (ctx) => {
        if (ctx.boxNamed("wood") || ctx.boxNamed("copper")) {
          return "This brace ends the block that created $n{wood} and $n{copper}, so both should be removed.";
        }
        if (!ctx.boxNamed("brick")) {
          return "$n{brick} was created outside this block, so it shouldn't be removed.";
        }
        return ctx.basicHint;
      },
    },
  ],
  next: "17-comparisons.html",
  workspace: { allowVariableCreation: true, allowVariableDeletion: true },
});
