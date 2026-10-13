import { createProgramTemplate } from "./shared-program-template.js";
createProgramTemplate({
    initialInstructions: "Each element of an array is a variable in its own right, with its own address. That means a pointer can point to a single element.\nClick $runLineButton to continue.",
    steps: [
        { code: "int a[3] = {4, 8, 15};\n" },
        {
            code: "int* p = &a[0];\n",
            instructions: "$c{&a[0]} is the address of $n{a}'s first element. Now $c{*p} is another way to refer to that element, just like $c{a[0]}.",
        },
        {
            code: "*p = 5;\n",
            editable: true,
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "p")) {
                    return "$c{*p = 5;} changes $n{*p}, not $n{p}. Click $resetButton and try again.";
                }
                return ctx.basicHint;
            },
        },
        {
            code: "p = &a[2];\n",
            editable: true,
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "p")) {
                    return "Set $n{p}'s value to the address of $c{a[2]}, which is shown above that element.";
                }
                return ctx.basicHint;
            },
        },
        {
            code: "*p += a[0];\n",
            editable: true,
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "a[2]")) {
                    return "$n{p} now points to $c{a[2]}, so $c{*p += a[0];} means $c{a[2] = a[2] + a[0];}.";
                }
                return ctx.basicHint;
            },
        },
        {
            code: "int* q = &a[1];\n",
            editable: true,
            hints: (ctx) => ctx.basicHint,
        },
        {
            code: "*q = *p - *q;\n",
            editable: true,
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "a[1]")) {
                    return "$n{*p} is $c{a[2]} and $n{*q} is $c{a[1]}.";
                }
                return ctx.basicHint;
            },
        },
    ],
    next: "84-pointer-arithmetic.html",
    workspace: { allowVariableCreation: true, showOtherNames: true },
});
