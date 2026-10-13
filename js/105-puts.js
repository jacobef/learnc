import { createProgramTemplate } from "./shared-program-template.js";
createProgramTemplate({
    implicitMain: false,
    initialInstructions: "$c{stdio.h} also declares $c{puts}, which prints a whole string, followed by a newline. Its declaration is $c{int puts(const char *s);}. The $c{const} means that $c{puts} promises not to change the characters it's given a pointer to; you can pass it an ordinary $t{char *}, which is what we'll do. (We'll cover $c{const} properly later.)\n$c{puts} takes a pointer to the first character of a string. It prints characters until it reaches $c{'\\0'}, then prints a newline.\nClick $runLineButton to continue.",
    steps: [
        { code: "#include <stdio.h>\n\n" },
        { code: "int main(void) {\n" },
        {
            code: "  puts(\"Hello\");\n",
            instructions: "The string literal is a nameless array, which decays into a pointer to its first character, the H. $c{puts} prints from there up to the null character, then adds a newline.",
        },
        { code: "  char word[] = \"abc\";\n" },
        {
            code: "  puts(word);\n",
            editable: true,
            hints: (ctx) => ctx.basicHint,
        },
        {
            code: "  puts(word + 1);\n",
            editable: true,
            instructions: "$c{puts} doesn't have to be given a pointer to the start of an array.",
            hints: (ctx) => {
                if (ctx.basicHint && ctx.output !== ctx.expectedOutput) {
                    return "$c{word + 1} points to $c{word[1]}, so $c{puts} starts printing from there.";
                }
                return ctx.basicHint;
            },
        },
        {
            code: "  word[1] = '\\0';\n",
            editable: true,
            hints: (ctx) => ctx.basicHint,
        },
        {
            code: "  puts(word);\n",
            editable: true,
            hints: (ctx) => {
                if (ctx.basicHint && ctx.output !== ctx.expectedOutput) {
                    return "$c{puts} stops at the first null character, which is now $c{word[1]}.";
                }
                return ctx.basicHint;
            },
        },
        { code: "  return 0;\n" },
        { code: "}\n" },
    ],
    next: "106-review-iv.html",
    workspace: { allowVariableCreation: true, showOutput: true },
});
