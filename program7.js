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
    ],
    editableSteps: [5, 6, 7],
    stepperFallback: false,
    next: "program8.html",
    workspace: { allowAddAndDelete: true },
    instructions: (ctx) => {
      if (ctx.boundary === 0) {
        return "This is where addresses become relevant.\nNo other instructions for this one. Good luck!";
      }
      return null;
    },
    hints: {
      5: (ctx) => {
        const by = ctx.byName || {};
        if (by.wolf.value !== by.hare.address) {
          return "$c{wolf = &deer;} set $n{wolf}'s value to $n{deer}'s address. What should $c{wolf = &hare;} do?";
        }
        return null;
      },
      6: (ctx) => {
        const by = ctx.byName || {};
        if (!by.bear) {
          return "You need to add the $n{bear} variable.";
        }
        if (by.bear.type !== "int**") {
          return "$n{bear}'s type should be $t{int**}.";
        }
        if (by.bear.value !== by.wolf.address) {
          return "Set $n{bear}'s value to $n{wolf}'s address.";
        }
        return null;
      },
      7: (ctx) => {
        const by = ctx.byName || {};
        if (!by.fox) {
          return "You need to add the $n{fox} variable.";
        }
        if (by.fox.type !== "int*") {
          return "$n{fox}'s type should be $t{int*}.";
        }
        if (by.fox.value !== by.wolf.value) {
          return "$n{fox}'s value should be set to $n{wolf}'s value.";
        }
        return null;
      },
    },
  });
}
