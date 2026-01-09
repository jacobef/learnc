{
  const { createProgramTemplate } = window.MB;

  createProgramTemplate({
    lines: ["int toaster;", "int fridge;", "toaster = 28;"],
    editableSteps: [2, 3],
    stepperFallback: true,
    next: "program3.html",
    workspace: { allowAddAndDelete: true },
    instructions: (ctx) => {
      if (ctx.boundary === 2) {
        return "$c{int fridge;} creates a new variable. Click $b{+ New variable} and enter its attributes.";
      }
      if (ctx.boundary === 3) {
        return "What does $c{toaster = 28;} do?";
      }
      return null;
    },
    hints: {
      2: (ctx) => {
        const boxes = ctx.boxes || [];
        if (boxes.length < 2) return "Read the instructions.";
        if (boxes.length > 2) {
          return "Only keep $n{toaster} and $n{fridge} in the program state.";
        }

        const toaster = ctx.byName.toaster;
        const fridge = boxes.find((b) => b.name !== "toaster");

        if (!fridge.name || fridge.name !== "fridge") {
          return "The new variable's name should be $n{fridge}.";
        }
        if (fridge.type !== "int") {
          return "$n{fridge}'s type should be $t{int}.";
        }
        if (fridge.value !== "") {
          return "$n{fridge} hasn't been assigned a value; its value should remain empty.";
        }
        if (toaster.value === "28") {
          return "$c{toaster = 28;} hasn't been run yet, so $n{toaster}'s value should be empty for now.";
        }
      },
      3: (ctx) => {
        const boxes = ctx.boxes || [];
        if (boxes.length < 2) return "Read the instructions.";
        if (boxes.length > 2) {
          return "Only keep $n{toaster} and $n{fridge} in the program state.";
        }
        const toaster = boxes.find((b) => b.name === "toaster");
        const fridge = boxes.find((b) => b.name === "fridge");
        if (fridge && fridge.value === "28" && toaster.value !== "28") {
          return "$v{28} belongs in $n{toaster}'s value, not $n{fridge}.";
        }
        if (fridge && fridge.value !== "") {
          return "This line doesn't change $n{fridge}. Leave its value empty.";
        }
        if (toaster.value !== "28") {
          return "Set $n{toaster}'s value to $v{28}.";
        }
      },
    },
  });
}
