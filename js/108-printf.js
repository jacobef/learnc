import { createProgramTemplate } from "./shared-program-template.js";
const output = (ctx) => ctx.basicHint;
createProgramTemplate({
    implicitMain: false,
    initialInstructions: "Congratulations on getting this far! You've learned the core of C, so the rest of this tutorial covers the rest of the language, starting with the function C programs use most to print things: $c{printf} (\"print formatted\"), declared in $c{stdio.h}.\n$c{printf}'s first argument is a string called the format string. It's printed as it is, except for conversion specifications, which start with $c{%}. Each one is replaced by the next of the remaining arguments, formatted according to the letter after the $c{%}.\nClick $runLineButton to continue.",
    steps: [
        { code: "#include <stdio.h>\n\n" },
        { code: "int main(void) {\n" },
        { code: "  int apples = 3;\n" },
        {
            code: "  printf(\"I have %d apples.\\n\", apples);\n",
            instructions: "$c{%d} formats an $t{int} in decimal, so it's replaced by $n{apples}'s value. Unlike $c{puts}, $c{printf} doesn't add a newline at the end, so the format string ends with $c{\\n}.",
        },
        {
            code: "  printf(\"%d + %d = %d\\n\", 2, 3, 2 + 3);\n",
            editable: true,
            instructions: "Each $c{%d} uses the next argument, in order. What does this print?",
            hints: output,
        },
        { code: "  char grade = 'B';\n" },
        {
            code: "  printf(\"Grade: %c\\n\", grade);\n",
            editable: true,
            instructions: "$c{%c} prints the character with the given code.",
            hints: output,
        },
        {
            code: "  printf(\"Name: %s\\n\", \"Ada\");\n",
            editable: true,
            instructions: "$c{%s} prints a string, given a pointer to its first character, just like $c{puts} but without the newline.",
            hints: output,
        },
        {
            code: "  double price = 2.5;\n",
        },
        {
            code: "  printf(\"Price: %f\\n\", price);\n",
            instructions: "$c{%f} prints a $t{double}, always with 6 digits after the decimal point. (We'll see how to change that next.)",
        },
        {
            code: "  printf(\"100%%\\n\");\n",
            editable: true,
            instructions: "Since $c{%} starts a conversion specification, printing an actual $c{%} takes $c{%%}.",
            hints: output,
        },
        {
            code: "  return 0;\n",
            instructions: "It's up to you to make the arguments match the format string: passing a $t{double} for $c{%d}, or too few arguments, is undefined behavior. (Compilers usually warn about it.)",
        },
        { code: "}\n" },
    ],
    next: "109-printf-formatting.html",
    workspace: { showOutput: true, hideStringLiterals: true },
});
