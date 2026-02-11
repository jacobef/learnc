import { createProgramTemplate } from "./shared-program-template.js";
createProgramTemplate({
    initialInstructions: "Click $runLineButton to continue.",
    steps: [
        {
            code: "int freezer;\n",
            instructions: "Click $runLineButton to continue.",
        },
        {
            code: "int fridge;\n",
            editable: true,
            instructions: "$c{int fridge;} creates a new variable. Click $newVariableButton and enter its attributes.",
            hints: (ctx) => {
                if (ctx.boxes.length < 2)
                    return "Read the instructions.";
                if (ctx.boxes.length > 2) {
                    return "Only keep $n{freezer} and $n{fridge} in the program state.";
                }
                const [freezer, fridge] = ctx.boxesNamed("freezer", "fridge");
                if (!fridge) {
                    return "The new variable's name should be $n{fridge}.";
                }
                if (fridge.type !== "int") {
                    return "$n{fridge}'s type should be $t{int}.";
                }
                if (fridge.value !== "") {
                    return "$n{fridge} hasn't been assigned a value; its value should remain empty.";
                }
                if (freezer.value === "28") {
                    return "$c{freezer = 28;} hasn't been run yet, so $n{freezer}'s value should be empty for now.";
                }
                return null;
            },
        },
        {
            code: "freezer = 28;\n",
            editable: true,
            instructions: "What does $c{freezer = 28;} do?",
            hints: (ctx) => {
                const { boxes, boxesNamed } = ctx;
                if (boxes.length > 2) {
                    return "Only keep $n{freezer} and $n{fridge} in the program state.";
                }
                const [freezer, fridge] = boxesNamed("freezer", "fridge");
                if (fridge.value === "28" && freezer.value !== "28") {
                    return "$v{28} belongs in $n{freezer}'s value, not $n{fridge}.";
                }
                if (fridge.value !== "") {
                    return "This line doesn't change $n{fridge}. Leave its value empty.";
                }
                if (freezer.value !== "28") {
                    return "Set $n{freezer}'s value to $v{28}.";
                }
                return null;
            },
        },
    ],
    next: "3-initialization.html",
    workspace: { allowVariableCreation: true },
});
