import { createProgramTemplate } from "./shared-program-template.js";
import { startArraysAmbiance } from "./shared-arrays-ambiance.js";
createProgramTemplate({
    initialInstructions: "So far, every variable has held a single value. An array holds several values of the same type, one after another in memory.\nBut a warning before we start. Almost everything in this tutorial so far has been a fairly simple idea that works the way you'd expect, everywhere you use it. Arrays are not like that. They're one of the most complicated parts of C: they come with a long list of special rules, and a lot of what you know about variables (like assigning them, copying them, and passing them around) works differently for arrays, or not at all.\nThe next several lessons go through arrays one piece at a time. If something about them seems strange, that's because it is strange, not because you've misunderstood.\nClick $runLineButton to continue.",
    steps: [
        {
            code: "int a[3];\n",
            instructions: "This creates an array named $n{a}: a single variable made of 3 $t{int}s, stored one after another in memory. These $t{int}s are called its elements. $n{a}'s type is $t{int[3]}, which means \"array of 3 $t{int}s\".\nNotice that the declaration is written $c{int a[3];}, not $c{int[3] a;}. We'll see why later.",
        },
        {
            code: "a[0] = 7;\n",
            instructions: "$c{a[0]} refers to the first element of $n{a}. The number in the brackets is called an index. Indices start at 0, not 1. (Why they start at 0 is one of the strange parts; we'll get to it.)",
        },
        {
            code: "a[2] = 9;\n",
            editable: true,
            instructions: "Since indices start at 0, $n{a}'s elements are $c{a[0]}, $c{a[1]}, and $c{a[2]}. Edit the values in the array box directly.",
            hints: (ctx) => {
                if (ctx.boxNamed("a[1]")?.value === "9") {
                    return "$c{a[2]} is the third element, not the second, since indices start at 0.";
                }
                return ctx.basicHint;
            },
        },
        {
            code: "a[1] = a[0] + a[2];\n",
            editable: true,
            hints: (ctx) => ctx.basicHint,
        },
        { code: "int i = 2;\n" },
        {
            code: "a[i] = 3;\n",
            editable: true,
            instructions: "An index can be any expression with an integer value, not just a number written directly.",
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "a[2]")) {
                    return "$n{i}'s value is $v{2}, so $c{a[i]} is $c{a[2]}.";
                }
                return ctx.basicHint;
            },
        },
        {
            code: "a[i - 2] += a[i];\n",
            editable: true,
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "a[0]")) {
                    return "$c{a[i - 2]} is $c{a[0]}, and $c{a[i]} is $c{a[2]}. Remember that $c{a[2]} was changed on line 6.";
                }
                return ctx.basicHint;
            },
        },
        {
            code: "int b[2];\n",
            editable: true,
            instructions: "Now create an array yourself: click $newVariableButton, then give it the name $n{b} and the type $t{int[2]}. Once you click away from the type, the box will turn into an array.",
            hints: (ctx) => {
                const b = ctx.boxNamed("b");
                if (b && !b.arrayShape?.length) {
                    return "$n{b}'s type should be $t{int[2]}. Click away from the type after typing it, and the box will turn into an array.";
                }
                return ctx.basicHint;
            },
        },
        {
            code: "b[1] = a[1] / 4;\n",
            editable: true,
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "b[1]")) {
                    return "$c{a[1]} is $v{16}, and this is $t{int} division.";
                }
                return ctx.basicHint;
            },
        },
    ],
    next: "77-arrays-ii.html",
    workspace: { allowVariableCreation: true },
    onOpenAtStart: startArraysAmbiance,
});
