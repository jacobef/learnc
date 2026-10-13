import { createProgramTemplate } from "./shared-program-template.js";
createProgramTemplate({
    initialInstructions: "Click $runLineButton to continue.",
    steps: [
        {
            code: "int c[3] = {5, 6, 7};\n",
            instructions: "An array can be initialized with a list of values in braces.",
        },
        {
            code: "int d[4] = {1, 2};\n",
            instructions: "If the list is shorter than the array, the remaining elements are set to $v{0}.",
        },
        {
            code: "int e[3] = {0};\n",
            editable: true,
            hints: (ctx) => {
                const e = ctx.boxNamed("e");
                if (e && !e.arrayShape?.length) {
                    return "$n{e} should be an array of 3 $t{int}s, so its type is $t{int[3]}.";
                }
                if (ctx.basicHintTopicIs("value")) {
                    return "The list sets $c{e[0]} to $v{0}, and the elements it doesn't mention are set to $v{0} too.";
                }
                return ctx.basicHint;
            },
        },
        {
            code: "int f[] = {3, 1, 4, 1, 5};\n",
            instructions: "If the size is left out, it's determined by how many values are in the list. $n{f}'s type is $t{int[5]}.",
        },
        {
            code: "double g[2] = {0.5};\n",
            editable: true,
            hints: (ctx) => {
                const g = ctx.boxNamed("g");
                if (g && !g.arrayShape?.length) {
                    return "$n{g} should be an array of 2 $t{double}s, so its type is $t{double[2]}.";
                }
                if (ctx.boxNamed("g[1]")?.value === "0") {
                    return "$c{g[1]} is a $t{double}, so it should be $v{0.0}, not $v{0}.";
                }
                return ctx.basicHint;
            },
        },
        {
            code: "c[0] = d[1] + f[4];\n",
            editable: true,
            instructions: "One thing you can't do is assign a whole array at once: $c{c = d;} isn't allowed, and won't compile. Arrays have to be copied one element at a time.",
            hints: (ctx) => ctx.basicHint,
        },
        {
            code: "d[3] = c[0] * e[2];\n",
            editable: true,
            hints: (ctx) => ctx.basicHint,
        },
    ],
    next: "78-indexing.html",
    workspace: { allowVariableCreation: true },
});
