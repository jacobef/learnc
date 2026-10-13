import { createProgramTemplate } from "./shared-program-template.js";
createProgramTemplate({
    implicitMain: false,
    initialInstructions: "So far, every block from $c{malloc} has leaked: it lasted until the program ended. That's fine for a short program, but a program that runs for a long time, like a web browser, has to give memory back when it's done with it. That's what $c{free} does.\nClick $runLineButton to continue.",
    steps: [
        { code: "#include <stdlib.h>\n\n" },
        { code: "int main(void) {\n" },
        { code: "  int *p = malloc(3 * sizeof(int));\n" },
        {
            code: "  if (p == NULL) {\n",
            instructions: "If there isn't enough memory, $c{malloc} returns $c{NULL} instead of an address. Real programs check for this, since using a $c{NULL} pointer is undefined behavior.",
        },
        { code: "    return 1;\n" },
        { code: "  }\n" },
        { code: "  p[0] = 5;\n" },
        {
            code: "  free(p);\n",
            instructions: "$c{free} takes a pointer to the start of a block from $c{malloc}, and ends the block's lifetime, so the memory can be reused. Notice that $n{p} still holds the same address: it's now a dangling pointer, pointing to memory that's no longer there. Using it in any way, even just comparing it, is undefined behavior.",
        },
        {
            code: "  p = NULL;\n",
            instructions: "Setting a pointer to $c{NULL} after freeing what it points to makes the mistake of using it again easier to catch. ($c{free(NULL)} does nothing, so it's safe to free a pointer that might be $c{NULL}.)",
        },
        { code: "  int *q = malloc(sizeof(int));\n" },
        { code: "  *q = 1;\n" },
        {
            code: "  free(q);\n",
            instructions: "Every block from $c{malloc} should be freed exactly once, when it's no longer needed.",
        },
        { code: "  return 0;\n" },
        { code: "}\n" },
    ],
    next: "135-memory-errors.html",
});
