import { createProgramTemplate } from "./shared-program-template.js";
createProgramTemplate({
    initialInstructions: "Every variable so far has been declared in the program, so its size and how long it lasts were fixed in the code. Sometimes a program needs memory it can ask for while it's running, for as long as it wants. The standard library function $c{malloc} (short for \"memory allocate\") does this.\nClick $runLineButton to continue.",
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
            instructions: "This block has room for 3 $t{int}s, one after another. A group of values of the same type, one after another in memory, is called an array, and each value in it is called an element. The elements are numbered starting from $v{0}, so this array's elements are $v{[0]}, $v{[1]}, and $v{[2]}. Its type is written $t{int[3]}: an array of 3 $t{int}s.",
        },
        {
            code: "*q = *p + 1;\n",
            editable: true,
            instructions: "$n{q} points to the array's first element, so $c{*q} is that element.",
            hints: (ctx) => ctx.basicHintTopicIs("value") ? "$c{*q} is element $v{[0]} of the array, and $c{*p} is $v{7}." : ctx.basicHint,
        },
        {
            code: "p = q;\n",
            instructions: "Now nothing points to the first block, but it still exists: a block from $c{malloc} lasts until the program says it's done with it (we'll see how later) or ends. Losing track of a block like this is called a memory leak. In a small program like this one, that's harmless, but a long-running program that keeps leaking memory will eventually run out.",
        },
    ],
    next: "77-pointer-arithmetic.html",
    workspace: { allowVariableCreation: true },
});
