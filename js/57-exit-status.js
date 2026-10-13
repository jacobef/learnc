import { createProgramTemplate } from "./shared-program-template.js";
createProgramTemplate({
    implicitMain: false,
    initialInstructions: "When $c{main} returns, the program ends, and the value $c{main} returns is handed to whoever ran the program, like the terminal or another program. This value is called the program's exit status. By convention, $v{0} means the program succeeded, and anything else means something went wrong; programs often use different numbers for different problems.\nThis program pretends to check something, and finds problems.\nClick $runLineButton to continue.",
    steps: [
        { code: "int main(void) {\n" },
        { code: "  int problems = 2;\n" },
        {
            code: "  if (problems > 0) {\n",
            editable: true,
            instructions: "Select where execution continues.",
        },
        {
            code: "    return 1;\n",
            instructions: "$c{main} returns here, so the rest of it never runs, and the program's exit status is $v{1}. A terminal can show it: after running a program, the command $c{echo $?} prints the last program's exit status.",
        },
        { code: "  }\n" },
        { code: "  return 0;\n" },
        { code: "}\n" },
    ],
    next: "58-recursion.html",
});
