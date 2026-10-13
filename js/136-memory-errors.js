import { createCompileQuizTemplate } from "./shared-compile-quiz-template.js";
const program = (body) => `#include <stdlib.h>\n\nint main(void) {\n${body}  return 0;\n}`;
createCompileQuizTemplate({
    implicitMain: false,
    allowUndefinedAnswer: true,
    initialInstructions: "Memory from $c{malloc} is one of the biggest sources of bugs in C programs, because the compiler can't check how it's used. These mistakes compile fine, and on a real computer, they often seem to work, until they cause a crash somewhere else much later. Some are also security holes.\nFor each program, decide what happens.",
    steps: [
        {
            code: program("  int *p = malloc(sizeof(int));\n  *p = 3;\n  free(p);\n  int x = *p;\n"),
            explanation: "This is called a use after free: $n{p} still holds the address, but the block is gone.",
        },
        {
            code: program("  int *p = malloc(sizeof(int));\n  free(p);\n  free(p);\n"),
            explanation: "This is a double free. Freeing a block that's already been freed is undefined behavior.",
        },
        {
            code: program("  int *p = malloc(3 * sizeof(int));\n  p[3] = 0;\n"),
            explanation: "The block only has room for 3 $t{int}s, so $c{p[3]} is past its end. This is called a buffer overflow.",
        },
        {
            code: program("  int *p = malloc(3);\n  p[1] = 0;\n"),
            explanation: "$c{malloc(3)} asks for 3 bytes, not 3 $t{int}s. That's not even enough for one $t{int}.",
            hint: "How many bytes does $c{malloc(3)} give?",
        },
        {
            code: program("  int *p = malloc(sizeof(int));\n  int x = *p + 1;\n"),
            explanation: "Memory from $c{malloc} starts out without values, just like an uninitialized variable. ($c{calloc} gives memory where every byte is zero.)",
        },
        {
            code: program("  int x = 5;\n  int *p = &x;\n  free(p);\n"),
            explanation: "$c{free} can only be given pointers from $c{malloc} (or $c{NULL}). $n{x} wasn't allocated, so it can't be freed.",
        },
        {
            code: program("  int *p = malloc(4 * sizeof(int));\n  free(p + 1);\n"),
            explanation: "$c{free} has to be given the pointer $c{malloc} returned, to the start of the block, not to somewhere inside it.",
        },
        {
            code: program("  for (int i = 0; i < 3; i++) {\n    int *p = malloc(sizeof(int));\n    *p = i;\n  }\n"),
            explanation: "This leaks three blocks, but leaking memory isn't undefined behavior. It's wasteful, though: nothing points to these blocks anymore, so they can never be freed.",
        },
    ],
    next: "137-realloc.html",
});
