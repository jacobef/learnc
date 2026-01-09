{
  const { createProgramTemplate } = window.MB;

  createProgramTemplate({
    lines: ["int m;", "m = 3;", "m = 5;"],
    editableSteps: [3],
    stepperFallback: "buttons",
    next: "program2.html",
    workspace: { allowAddAndDelete: false },
    instructions: (ctx) => {
      if (ctx.boundary === 3) {
        return "Edit the program state to what it should be after line 3 is run, then press $b{Check}.";
      }
      return null;
    },
    hints: {
      3: () =>
        "Line 2 ($c{m = 3;}) set $n{m}'s value to $v{3}, so what should line 3 ($c{m = 5;}) do?",
    },
  });
}
