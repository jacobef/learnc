{
  const { createProgramTemplate } = window.MB;

  createProgramTemplate({
    lines: [
      "int deer;",
      "int hare;",
      "int* wolf;",
      "wolf = &deer;",
      "wolf = &hare;",
      "int** bear = &wolf;",
      "int* fox = wolf;",
      "deer = 50;",
      "*wolf = 11;",
      "*bear = &deer;",
      "int elk = *wolf;",
      "*fox = **bear;",
    ],
    editableSteps: [7, 9, 10, 11, 12],
    stepperFallback: false,
    next: "dereferencing-quiz.html",
    workspace: { allowAddAndDelete: true, showOtherNames: true },
    instructions: (ctx) => {
      if (ctx.boundary === 0) {
        return "Let's revisit 7, but with some lines added at the end. To understand these new lines, we need a better understanding of what was going on in 7. Click $b{Run line 1 ▶} to continue.";
      }
      if (ctx.boundary === 4) {
        return "When $n{wolf} is assigned to $c{&deer}, $n{wolf}'s value becomes $n{deer}'s address. Also, $n{deer} gains an additional name. Use the $b{Show aliases} toggle under $n{deer} to reveal this name.\n\nWe say that $n{wolf} now \"points to\" $n{deer}.";
      }
      if (ctx.boundary === 5) {
        return 'When $n{wolf} is assigned to $c{&hare}, the $n{*wolf} name moves from $n{deer} to $n{hare}. Use the $b{Show aliases} toggle under $n{hare} to reveal it. We say that $n{wolf} now "points to" $n{hare}.\n\nIn general, if some variable $n{X} points to another variable $n{Y}, then $n{*X} refers to $n{Y}. In this case, $n{wolf} points to $n{hare}, so $n{*wolf} refers to $n{hare}.\n\nWe\'ll see the relevance of this alternate name later in the code.';
      }
      if (ctx.boundary === 6) {
        return "$c{bear = &wolf;} adds the $n{*bear} name to $n{wolf}. It also adds a name to $n{hare}; use the $b{Show aliases} toggle under $n{hare} to reveal it.\n\nTo understand why: $n{*bear} points to $n{hare} (because $n{*bear} now refers to $n{wolf}, and $n{wolf} points to $n{hare}). Therefore, we can refer to $n{hare} by adding another asterisk to $n{*bear}, which gives us $n{**bear}.";
      }
      if (ctx.boundary === 7) {
        return "You're on your own now, good luck! The aliases will update automatically as you make edits.";
      }
      return null;
    },
    hints: {
      7: (ctx) => {
        const by = ctx.byName || {};
        if (by.deer.value !== "" || by.hare.value !== "") {
          return "$n{deer} and $n{hare} haven't stored values yet—leave them empty.";
        }
        if (by.bear.value !== by.wolf.address) {
          return "$n{bear}'s value should be left unchanged. Click $b{Reset} and try again.";
        }
        if (!by.fox) {
          return "Add the $n{fox} variable.";
        }
        if (by.fox.type !== "int*") {
          return "$n{fox}'s type should be $t{int*}.";
        }
        if (by.fox.value !== by.wolf.value) {
          return "$n{fox}'s value should be set to $n{wolf}'s value.";
        }
        return null;
      },
      9: (ctx) => {
        const by = ctx.byName || {};
        if (by.hare.value !== "11" && by.wolf.value === "11") {
          return "Set $n{*wolf}'s value to $v{11}, not $n{wolf}'s value. $n{*wolf} refers to the variable whose address is $n{wolf}'s value. Click $b{Reset} and try again.";
        }
        if (by.hare.value !== "11") {
          return "$n{*wolf} refers to the variable whose address is $n{wolf}'s value. You need to set $n{*wolf}'s value to $v{11}.";
        }
        return null;
      },
      10: (ctx) => {
        const by = ctx.byName || {};
        if (by.wolf.value !== by.deer.address) {
          return "$n{*bear} refers to the variable whose address is $n{bear}'s value. $c{&deer} refers to $n{deer}'s address.";
        }
        return null;
      },
      11: (ctx) => {
        const by = ctx.byName || {};
        if (!by.elk) {
          return "Add the $n{elk} variable.";
        }
        if (by.elk.type !== "int") {
          return "$n{elk}'s type should be $t{int}.";
        }
        if (by.elk.value !== by.deer.value) {
          return "$n{elk}'s value should be set to $n{*wolf}'s value. $n{*wolf} refers to the variable whose address is $n{wolf}'s value.";
        }
        return null;
      },
      12: (ctx) => {
        const by = ctx.byName || {};
        if (by.hare.value !== by.deer.value) {
          return "$n{*fox} refers to the variable whose address is $n{fox}'s value. $n{**bear} refers to the variable whose address is $n{*bear}'s value, and $n{*bear} refers to the variable whose address is $n{bear}'s value.";
        }
        return null;
      },
    },
  });
}
