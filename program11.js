{
  const { createProgramTemplate } = window.MB;

  createProgramTemplate({
    lines: [
      "int yin = 2;",
      "int yang = 5;",
      "yin = yang + yin;",
      "yang = yang + 1;",
      "yin = yin;",
      "yang = yang-yang * 2;",
    ],
    editableSteps: [4, 5, 6],
    stepperFallback: false,
    next: "program12.html",
    workspace: { allowAddAndDelete: true },
    instructions: (ctx) => {
      if (ctx.boundary === 0) {
        return "No instructions for this one. Good luck!";
      }
      return null;
    },
    hints: {
      4: (ctx) => {
        const by = ctx.byName || {};
        if (by.yin.value !== "7") {
          return "$n{yin}'s value should remain $v{7}.";
        }
        return "Take the current value of $n{yang}, then add 1. That result should be the new value of $n{yang}.";
      },
      5: (ctx) => {
        return "This line assigns $n{yin} to itself, so nothing should change.";
      },
      6: (ctx) => {
        const by = ctx.byName || {};
        if (by.yang.value === "0") {
          return "Haha, gotcha. Always remember order of operations.";
        }
        return "Multiply before subtracting, and update $n{yang} accordingly.";
      },
    },
  });
}
