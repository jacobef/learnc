import { createProgramTemplate } from "./shared-program-template.js";

createProgramTemplate({
  initialInstructions: "Click $runLineButton to continue.",
  steps: [
    {
      code: "int toaster;\n",
      editable: false,
      instructions: "Click $runLineButton to continue.",
    },
    {
      code: "int fridge;\n",
      editable: true,
      instructions:
        "$c{int fridge;} creates a new variable. Click $newVariableButton and enter its attributes.",
      hints: (ctx) => {
        if (ctx.boxes.length < 2) return "Read the instructions.";
        if (ctx.boxes.length > 2) {
          return "Only keep $n{toaster} and $n{fridge} in the program state.";
        }

        const [toaster, fridge] = ctx.boxesNamed("toaster", "fridge");
        if (!fridge) {
          return "The new variable's name should be $n{fridge}.";
        }
        if (fridge.type !== "int") {
          return "$n{fridge}'s type should be $t{int}.";
        }
        if (fridge.value !== "") {
          return "$n{fridge} hasn't been assigned a value; its value should remain empty.";
        }
        if (toaster!.value === "28") {
          return "$c{toaster = 28;} hasn't been run yet, so $n{toaster}'s value should be empty for now.";
        }
        return null;
      },
    },
    {
      code: "toaster = 28;\n",
      editable: true,
      instructions: "What does $c{toaster = 28;} do?",
      hints: (ctx) => {
        const { boxes, boxesNamed } = ctx;
        if (boxes.length > 2) {
          return "Only keep $n{toaster} and $n{fridge} in the program state.";
        }
        const [toaster, fridge] = boxesNamed("toaster", "fridge");
        if (fridge!.value === "28" && toaster!.value !== "28") {
          return "$v{28} belongs in $n{toaster}'s value, not $n{fridge}.";
        }
        if (fridge!.value !== "") {
          return "This line doesn't change $n{fridge}. Leave its value empty.";
        }
        if (toaster!.value !== "28") {
          return "Set $n{toaster}'s value to $v{28}.";
        }
        return null;
      },
    },
  ],
  next: "program3.html",
  workspace: { allowVariableCreation: true },
});
