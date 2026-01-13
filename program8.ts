/// <reference path="./shared-program-template.ts" />

{
  const createProgramTemplate = window.MB!.createProgramTemplate!;

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
          "When $n{wolf} is assigned to $c{&deer}, $n{wolf}'s value becomes $n{deer}'s address. Also, $n{deer} gains an additional name. Use the $showAliasesButton toggle under $n{deer} to reveal this name.\n\nWe say that $n{wolf} now \"points to\" $n{deer}.",
      },
      {
        code: "wolf = &hare;\n",
        editable: false,
        instructions:
          'When $n{wolf} is assigned to $c{&hare}, the $n{*wolf} name moves from $n{deer} to $n{hare}. Use the $showAliasesButton toggle under $n{hare} to reveal it. We say that $n{wolf} now "points to" $n{hare}.\n\nIn general, if some variable $n{X} points to another variable $n{Y}, then $n{*X} refers to $n{Y}. In this case, $n{wolf} points to $n{hare}, so $n{*wolf} refers to $n{hare}.\n\nWe\'ll see the relevance of this alternate name later in the code.',
      },
      {
        code: "int** bear = &wolf;\n",
        editable: false,
        instructions:
          "$c{bear = &wolf;} adds the $n{*bear} name to $n{wolf}. It also adds a name to $n{hare}; use the $showAliasesButton toggle under $n{hare} to reveal it.\n\nTo understand why: $n{*bear} points to $n{hare} (because $n{*bear} now refers to $n{wolf}, and $n{wolf} points to $n{hare}). Therefore, we can refer to $n{hare} by adding another asterisk to $n{*bear}, which gives us $n{**bear}.",
      },
      {
        code: "int* fox = wolf;\n",
        editable: true,
        instructions:
          "You're on your own now, good luck! The aliases will update automatically as you make edits.",
        hints: (ctx) => {
          const [deer, hare, bear, wolf] = ctx.getBoxesByName(
            ctx.boxes,
            "deer",
            "hare",
            "bear",
            "wolf",
          );
          if (deer!.value !== "" || hare!.value !== "") {
            return "$n{deer} and $n{hare} haven't stored values yet—leave them empty.";
          }
          if (bear!.value !== wolf!.address) {
            return "$n{bear}'s value should be left unchanged. Click $resetButton and try again.";
          }
          const fox = ctx.getBoxByName(ctx.boxes, "fox");
          if (!fox) {
            return "Add the $n{fox} variable.";
          }
          if (fox.type !== "int*") {
            return "$n{fox}'s type should be $t{int*}.";
          }
          if (fox.value !== wolf!.value) {
            return "$n{fox}'s value should be set to $n{wolf}'s value.";
          }
          return null;
        },
      },
      { code: "deer = 50;\n", editable: false },
      {
        code: "*wolf = 11;\n",
        editable: true,
        hints: (ctx) => {
          const [hare, wolf] = ctx.getBoxesByName(ctx.boxes, "hare", "wolf");
          if (hare!.value !== "11" && wolf!.value === "11") {
            return "Set $n{*wolf}'s value to $v{11}, not $n{wolf}'s value. $n{*wolf} refers to the variable whose address is $n{wolf}'s value. Click $resetButton and try again.";
          }
          if (hare!.value !== "11") {
            return "$n{*wolf} refers to the variable whose address is $n{wolf}'s value. You need to set $n{*wolf}'s value to $v{11}.";
          }
          return null;
        },
      },
      {
        code: "*bear = &deer;\n",
        editable: true,
        hints: (ctx) => {
          const [wolf, deer] = ctx.getBoxesByName(ctx.boxes, "wolf", "deer");
          if (wolf!.value !== deer!.address) {
            return "$n{*bear} refers to the variable whose address is $n{bear}'s value. $c{&deer} refers to $n{deer}'s address.";
          }
          return null;
        },
      },
      {
        code: "int elk = *wolf;\n",
        editable: true,
        hints: (ctx) => {
          const [elk, deer] = ctx.getBoxesByName(ctx.boxes, "elk", "deer");
          if (!elk) {
            return "Add the $n{elk} variable.";
          }
          if (elk.type !== "int") {
            return "$n{elk}'s type should be $t{int}.";
          }
          if (elk.value !== deer!.value) {
            return "$n{elk}'s value should be set to $n{*wolf}'s value. $n{*wolf} refers to the variable whose address is $n{wolf}'s value.";
          }
          return null;
        },
      },
      {
        code: "*fox = **bear;\n",
        editable: true,
        hints: (ctx) => {
          const [hare, deer] = ctx.getBoxesByName(ctx.boxes, "hare", "deer");
          if (hare!.value !== deer!.value) {
            return "$n{*fox} refers to the variable whose address is $n{fox}'s value. $n{**bear} refers to the variable whose address is $n{*bear}'s value, and $n{*bear} refers to the variable whose address is $n{bear}'s value.";
          }
          return null;
        },
      },
    ],
    next: "dereferencing-quiz.html",
    workspace: { allowVariableCreation: true, showOtherNames: true },
  });
}
