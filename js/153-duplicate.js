import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";
createCodeOutputChallengeTemplate({
    inputs: [{ name: "word", type: "char[]" }],
    outputs: [
        { name: "same", type: "int" },
        { name: "separate", type: "int" },
    ],
    testInputs: ['"hello"', '""', '"a"', '"C is fun"'].map((word) => [word]),
    startInput: ['"hello"'],
    prelude: "#include <stdlib.h>\n#include <string.h>\n",
    main: ([word]) => `int main(void) {\n  char word[] = ${word};\n  char *copy = duplicate(word);\n  int same = strcmp(copy, word) == 0;\n  int separate = copy != word;\n  free(copy);\n  return 0;\n}`,
    solve: "char *duplicate(const char *s) {\n  char *copy = malloc(strlen(s) + 1);\n  strcpy(copy, s);\n  return copy;\n}",
    textareaMinLines: 6,
    instructions: "Write a function named $c{duplicate} that takes a pointer to a string's first character, and returns a pointer to a new copy of the string, in a block from $c{malloc}. The caller will $c{free} it.\nRemember to leave room for the $c{'\\0'}.",
    hints: (ctx) => {
        if (ctx.currentResult.kind === "ub")
            return "Is the block big enough for every character, plus the $c{'\\0'}?";
        if (ctx.currentResult.kind === "wrong-output-value") {
            return "The copy has to be a separate array, from $c{malloc}, not the same pointer you were given.";
        }
        return null;
    },
    next: "154-input.html",
});
