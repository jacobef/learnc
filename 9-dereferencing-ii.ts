import { createProgramTemplate } from "./shared-program-template.js";

createProgramTemplate({
  initialInstructions:
    "Let's revisit 7, but with some lines added at the end. To understand these new lines, we need a better understanding of what was going on in 7. Click $runLineButton to continue.",
  steps: [
    {
      code: "int deer;\n",
      editable: false,
    },
    { code: "int hare;\n", editable: false },
    { code: "int* wolf;\n", editable: false },
    {
      code: "wolf = &deer;\n",
      editable: false,
      instructions:
        "When $n{wolf} is assigned to $c{&deer}, $n{deer} gains an additional name. Use the $showAliasesButton button under $n{deer} to reveal this name.",
    },
    {
      code: "wolf = &hare;\n",
      editable: false,
      instructions:
        "When $n{wolf} is assigned to $c{&hare}, the $n{*wolf} name moves from $n{deer} to $n{hare}.",
    },
    {
      code: "int** bear;\n",
      editable: false,
    },
    {
      code: "bear = &wolf;\n",
      editable: false,
      instructions:
        "$c{bear = &wolf;} adds names to $n{wolf} and $n{hare}. Use the $showAliasesButton buttons under those variables to reveal them.",
    },
    {
      code: "int* fox = wolf;\n",
      editable: true,
      instructions:
        "You're on your own now, good luck! The aliases will update automatically as you make edits.",
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "fox")) {
          return "$n{fox}'s value should be set to $n{wolf}'s value.";
        }
        return ctx.basicHint;
      },
    },
    { code: "deer = 50;\n", editable: false },
    {
      code: "*wolf = 11;\n",
      editable: true,
      hints: (ctx) => {
        const [hare, wolf] = ctx.boxesNamed("hare", "wolf");
        if (hare!.value !== "11" && wolf!.value === "11") {
          return "Set $n{*wolf}'s value to $v{11}, not $n{wolf}'s value. $n{*wolf} refers to the variable whose address is $n{wolf}'s value. Click $resetButton and try again.";
        }
        if (hare!.value !== "11") {
          return "$n{*wolf} refers to the variable whose address is $n{wolf}'s value. You need to set $n{*wolf}'s value to $v{11}.";
        }
        return ctx.basicHint;
      },
    },
    {
      code: "*bear = &deer;\n",
      editable: true,
      hints: (ctx) => {
        const [wolf, deer] = ctx.boxesNamed("wolf", "deer");
        if (wolf!.value !== deer!.address) {
          return "$n{*bear} refers to the variable whose address is $n{bear}'s value. $c{&deer} refers to $n{deer}'s address.";
        }
        return ctx.basicHint;
      },
    },
    {
      code: "int elk = *wolf;\n",
      editable: true,
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "elk")) {
          const [elk, wolf] = ctx.boxesNamed("elk", "wolf");
          if (elk!.value === wolf!.value) {
            return "$n{elk}'s value should be set to $n{*wolf}'s value, not $n{wolf}'s value. $n{*wolf} refers to the variable whose address is $n{wolf}'s value.";
          }
          return "$n{elk}'s value should be set to $n{*wolf}'s value. $n{*wolf} refers to the variable whose address is $n{wolf}'s value.";
        }
        return ctx.basicHint;
      },
    },
    {
      code: "*fox = **bear;\n",
      editable: true,
      hints: (ctx) => {
        const [hare, deer] = ctx.boxesNamed("hare", "deer");
        if (hare!.value !== deer!.value) {
          return "$n{*fox} refers to the variable whose address is $n{fox}'s value. $n{**bear} refers to the variable whose address is $n{*bear}'s value, and $n{*bear} refers to the variable whose address is $n{bear}'s value.";
        }
        return ctx.basicHint;
      },
    },
  ],
  next: "10-whitespace-comments.html",
  workspace: { allowVariableCreation: true, showOtherNames: true },
});
