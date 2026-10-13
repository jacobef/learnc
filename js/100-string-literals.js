import { createProgramTemplate } from "./shared-program-template.js";
createProgramTemplate({
    initialInstructions: "A string literal that isn't initializing an array is different: it's a $t{char} array in its own right. That array has no name, and it exists for the whole time the program runs, which is why it's already in the program state before any lines have run.\nClick $runLineButton to continue.",
    steps: [
        {
            code: "char *p = \"cat\";\n",
            instructions: "Like any array used in an expression, the string literal decays into a pointer to its first element, the $c{'c'}. So $n{p} points to the nameless array's first element.",
        },
        {
            code: "char first = *p;\n",
            editable: true,
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "first")) {
                    return "$n{*p} is the first element of the nameless array.";
                }
                return ctx.basicHint;
            },
        },
        {
            code: "char last = p[2];\n",
            editable: true,
            hints: (ctx) => ctx.basicHint,
        },
        {
            code: "char s[] = \"cat\";\n",
            instructions: "Compare this with $n{s}: $c{\"cat\"} is initializing an array here, so it's only shorthand for $c{\\{'c', 'a', 't', '\\0'\\}}. $n{s} is an ordinary array, separate from the nameless one.",
        },
        {
            code: "s[0] = 'b';\n",
            editable: true,
            instructions: "Changing $n{s} is fine. But a string literal's array must never be changed: $c{p[0] = 'b';} would be undefined behavior.",
            hints: (ctx) => ctx.basicHint,
        },
        {
            code: "p = s;\n",
            editable: true,
            instructions: "$n{p} is an ordinary pointer, so it can be changed to point somewhere else, like $n{s}'s first element.",
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "p")) {
                    return "$c{s} decays into a pointer to its first element, $c{s[0]}.";
                }
                return ctx.basicHint;
            },
        },
        {
            code: "char *q = \"dog\";\n",
            editable: true,
            instructions: "This program contains a second string literal, which has its own nameless array.",
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "q")) {
                    return "$n{q} should point to the first element of the nameless array holding $c{\"dog\"}.";
                }
                return ctx.basicHint;
            },
        },
    ],
    next: "101-array-rules.html",
    workspace: { allowVariableCreation: true },
});
