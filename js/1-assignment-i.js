import { createProgramTemplate } from "./shared-program-template.js";
createProgramTemplate({
    initialInstructions: "Use the $backButton and $runLineButton buttons, or the left and right arrow keys, to see how the code changes the program state.",
    steps: [
        {
            code: "int m;\n",
            editable: false,
        },
        {
            code: "m = 3;\n",
            editable: false,
        },
        {
            code: "m = 5;\n",
            editable: true,
            instructions: "Edit the program state to what it should be after line 3 is run, then press $checkButton.",
            hints: () => "Line 2 ($c{m = 3;}) set $n{m}'s value to $v{3}, so what should line 3 ($c{m = 5;}) do?",
        },
    ],
    next: "2-declaration.html",
    workspace: { allowVariableCreation: false },
});
