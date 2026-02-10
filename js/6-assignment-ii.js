import { createProgramTemplate } from "./shared-program-template.js";
createProgramTemplate({
    initialInstructions: "No instructions for this one. Good luck!",
    steps: [
        { code: "int hammer;\n" },
        { code: "int nail = 1;\n" },
        { code: "hammer = nail;\n" },
        { code: "nail = 2;\n" },
        {
            code: "nail = hammer;\n",
            editable: true,
            hints: (ctx) => {
                const [nail, hammer] = ctx.boxesNamed("nail", "hammer");
                if (nail.value.toLowerCase() === "hammer") {
                    return `$n{nail}'s value should be set to $n{hammer}'s value, not the literal word "hammer".`;
                }
                if (hammer.value !== "1") {
                    return "$c{nail = hammer;} should modify $n{nail}, not $n{hammer}. Click $resetButton and try again.";
                }
                if (nail.value !== "1") {
                    return "$c{hammer = nail;} put $n{nail}'s value into $n{hammer}. What should $c{nail = hammer;} do?";
                }
                return ctx.basicHint;
            },
        },
    ],
    next: "7-pointers.html",
    workspace: { allowVariableCreation: true },
});
