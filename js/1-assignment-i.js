import { createProgramTemplate } from "./shared-program-template.js";
createProgramTemplate({
    initialInstructions: "Click $runLineButton to continue.",
    steps: [
        {
            code: "int m;\n",
            instructions: "Use the $backButton and $runLineButton buttons, or the left and right arrow keys, to see how the code changes the program state.",
        },
        {
            code: "m = 3;\n",
        },
        {
            code: "m = 5;\n",
        },
        {
            code: "m = 0;\n",
            editable: true,
            instructions: "Edit the program state to what it should be after line 4 is run, then press $checkButton.",
            hints: () => "Line 3 ($c{m = 5;}) set $n{m}'s value to $v{5}, so what should line 4 ($c{m = 0;}) do?",
        },
    ],
    next: "2-declaration.html",
    workspace: { allowVariableCreation: false },
});
