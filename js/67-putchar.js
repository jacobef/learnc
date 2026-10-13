import { createProgramTemplate } from "./shared-program-template.js";
const outputHint = (ctx) => ctx.basicHint;
createProgramTemplate({
    implicitMain: false,
    initialInstructions: "So far, programs have only stored values in variables. To show anything to a person, a program has to send it to an output. Every C program has an output called standard output, or stdout; when you run a program in a terminal, stdout is what appears in the terminal. This tutorial shows it in the Output panel.\n$c{putchar} is a function that's already written for you, as part of C's standard library. It writes one character to stdout.\nClick $runLineButton to continue.",
    steps: [
        { code: "int putchar(int c);\n\n" },
        { code: "int main(void) {\n" },
        {
            code: "  putchar(72);\n",
            instructions: "Like any function, $c{putchar} has to be declared before it's used; line 1 is its declaration. Its definition isn't in this program: it's part of the standard library, which was compiled ahead of time and gets combined with every C program. It takes the character's code as an $t{int}. (It also returns an $t{int}, which this program ignores. A function call's value doesn't have to be used.)\nThe character with code $v{72} is H.",
        },
        {
            code: "  putchar('i');\n",
            editable: true,
            instructions: "Now the output gets edited along with the program state. Type what the output should be after this line runs.",
            hints: outputHint,
        },
        {
            code: "  putchar('\\n');\n",
            editable: true,
            instructions: "Printing the newline character moves the cursor to the start of the next line. To type a newline into the output, press Enter.",
            hints: outputHint,
        },
        { code: "  char c = 'a';\n" },
        {
            code: "  while (c <= 'e') {\n    putchar(c);\n    c++;\n  }\n",
            editable: true,
            instructions: "What should the program state and output look like after this loop finishes?",
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "c")) {
                    return "The loop keeps going until $n{c} is greater than $c{'e'}.";
                }
                return ctx.basicHint;
            },
        },
        {
            code: "  putchar('\\n');\n",
            editable: true,
            hints: outputHint,
        },
        { code: "  return 0;\n" },
        { code: "}\n" },
    ],
    next: "68-stars.html",
    workspace: { allowVariableCreation: true, showOutput: true },
});
