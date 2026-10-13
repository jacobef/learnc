import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";
createCodeOutputChallengeTemplate({
    checkStdout: true,
    implicitMain: false,
    solve: "#include <stdio.h>\n\nint main(void) {\n  puts(\"Hello, World!\");\n  return 0;\n}\n",
    textareaMinLines: 8,
    instructions: "Time for the most famous program in programming. Write, from scratch, a complete C program that prints $c{Hello, World!} followed by a newline.\nIt's only a few lines long, but you now know what every part of it means: the preprocessor directive that declares $c{puts}, the $c{main} function where the program starts, the string literal (a nameless array, ending in $c{'\\0'}) that decays into a pointer to its first character, and the $c{return 0;} that tells whoever ran the program that it succeeded.",
    hints: (ctx) => {
        if (ctx.currentResult.kind === "wrong-stdout") {
            return "Check the output exactly: capital H and W, a comma, a space, an exclamation mark, then a newline.";
        }
        return null;
    },
    next: "108-printf.html",
});
