import { createProgramTemplate } from "./shared-program-template.js";
createProgramTemplate({
    initialInstructions: "Every variable so far, arrays included, has been declared in the program. So its size was fixed when the program was written, and it's removed at the end of its block. Sometimes a program needs memory it can ask for while it's running: as much as it turns out to need, for as long as it wants. The standard library function $c{malloc} (short for \"memory allocate\") does this.\nClick $runLineButton to continue.",
    steps: [
        {
            code: "#include <stdlib.h>\n",
            instructions: "$c{malloc} is declared in $c{stdlib.h}, the standard library's general-purpose header.",
        },
        {
            code: "int* p = malloc(sizeof(int));\n",
            instructions: "$c{malloc} finds an unused block of memory with the number of bytes it's asked for, and returns the address of the block's first byte. Here, that's enough bytes for one $t{int}.\nThe block isn't a variable: it has no name, and the only way to reach it is through a pointer to it. It doesn't even have a type of its own. It takes on the type of whatever's stored in it, so it's shown as the type $n{p} points to.\n($c{malloc} returns a $t{void*}: a pointer to no type in particular. It's automatically converted to the pointer type it's assigned to.)",
        },
        {
            code: "*p = 7;\n",
            editable: true,
            hints: (ctx) => ctx.basicHintTopicIs("value") ? "$c{*p} is the block that $n{p} points to." : ctx.basicHint,
        },
        {
            code: "int* q = malloc(3 * sizeof(int));\n",
            instructions: "This block has room for 3 $t{int}s, one after another, so it's an array of 3 $t{int}s, just like $c{int a[3];} would make, except that it has no name. $n{q} points to its first element, so it's used exactly like an array that has decayed into a pointer: $c{q[1]} is its second element.",
        },
        {
            code: "q[1] = *p + 1;\n",
            editable: true,
            hints: (ctx) => ctx.basicHintTopicIs("value") ? "$c{q[1]} is element $v{[1]} of the array, and $c{*p} is $v{7}." : ctx.basicHint,
        },
        {
            code: "p = q;\n",
            instructions: "Now nothing points to the first block, but it still exists: a block from $c{malloc} lasts until the program says it's done with it (we'll see how later) or ends. Losing track of a block like this is called a memory leak. In a small program like this one, that's harmless, but a long-running program that keeps leaking memory will eventually run out.",
        },
    ],
    next: "89-allocated-loops.html",
    workspace: { allowVariableCreation: true },
});
