import { createProgramTemplate } from "./shared-program-template.js";
createProgramTemplate({
    implicitMain: false,
    initialInstructions: "A block from $c{malloc} can't grow, but $c{realloc} can give you a bigger one with the same contents. It's how programs build arrays when they don't know in advance how many elements there will be.\nClick $runLineButton to continue.",
    steps: [
        { code: "#include <stdlib.h>\n\n" },
        { code: "int main(void) {\n" },
        { code: "  int *a = malloc(2 * sizeof(int));\n" },
        { code: "  a[0] = 10;\n" },
        { code: "  a[1] = 20;\n" },
        {
            code: "  int *bigger = realloc(a, 4 * sizeof(int));\n",
            instructions: "$c{realloc} makes a new block of the requested size, copies the old block's contents into it, frees the old block, and returns the new block's address. (Sometimes it can just grow the old block in place, but you can't count on that.) Now $n{a} is a dangling pointer.\nIf there isn't enough memory, $c{realloc} returns $c{NULL} and leaves the old block alone, which is why its result goes in a new variable first.",
        },
        { code: "  if (bigger == NULL) {\n" },
        { code: "    free(a);\n" },
        { code: "    return 1;\n" },
        { code: "  }\n" },
        { code: "  a = bigger;\n" },
        { code: "  a[2] = 30;\n" },
        { code: "  free(a);\n" },
        { code: "  return 0;\n" },
        { code: "}\n" },
    ],
    next: "152-string-functions.html",
});
