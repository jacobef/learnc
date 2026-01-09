{
  const { createProgramTemplate } = window.MB;

  createProgramTemplate({
    lines: [
      "int hammer;",
      "int drill = 1;",
      "hammer = drill;",
      "drill = 2;",
      "drill = hammer;",
    ],
    editableSteps: [5],
    stepperFallback: false,
    next: "program7.html",
    workspace: { allowAddAndDelete: true },
    instructions: (ctx) => {
      if (ctx.boundary === 0) {
        return "No instructions for this one. Good luck!";
      }
      return null;
    },
    hints: {
      5: (ctx) => {
        const by = ctx.byName || {};
        if (by.drill.value.trim().toLowerCase() === "hammer") {
          return `$n{drill}'s value should be set to $n{hammer}'s value, not the literal word "hammer".`;
        }
        if (by.hammer.value !== "1") {
          return "$c{drill = hammer;} should modify $n{drill}, not $n{hammer}. Click $b{Reset} and try again.";
        }
        if (by.drill.value !== "1") {
          return "$c{hammer = drill;} put $n{drill}'s value into $n{hammer}. What should $c{drill = hammer;} do?";
        }
        return null;
      },
    },
  });
}
