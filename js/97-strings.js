import { createProgramTemplate } from "./shared-program-template.js";
createProgramTemplate({
    initialInstructions: "Click $runLineButton to continue.",
    steps: [
        {
            code: "char s[6] = {'h', 'e', 'l', 'l', 'o', '\\0'};\n",
            instructions: "In C, a string (a piece of text) is stored as an array of $t{char}s holding the text's characters, followed by a null character, $c{'\\0'}. The null character marks where the string ends, so that code reading the string knows when to stop.",
        },
        {
            code: "char t[] = \"hello\";\n",
            instructions: "This does exactly the same thing as line 1. When a string literal like $c{\"hello\"} is used to initialize a $t{char} array, it's shorthand for a list of its characters, followed by $c{'\\0'}. That's why $n{t} has 6 elements, not 5.",
        },
        {
            code: "t[0] = 'j';\n",
            editable: true,
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "t[0]")) {
                    return "$c{'j'} is the code for the letter j, which comes 2 letters after $c{'h'}. The $referenceButton lists the code of every letter.";
                }
                return ctx.basicHint;
            },
        },
        {
            code: "t[4] = '\\0';\n",
            editable: true,
            instructions: "Now $n{t} holds the string \"jell\": anything after the first null character isn't part of the string.",
            hints: (ctx) => ctx.basicHint,
        },
        {
            code: "char u[8] = \"hi\";\n",
            editable: true,
            instructions: "The array can be bigger than the string. Recall that elements left out of an initializer are set to $v{0}.",
            hints: (ctx) => {
                const u = ctx.boxNamed("u");
                if (u && !u.arrayShape?.length) {
                    return "$n{u} is an array of 8 $t{char}s, so its type is $t{char[8]}.";
                }
                return ctx.basicHint;
            },
        },
        { code: "int length = 0;\n" },
        {
            code: "while (u[length] != '\\0') {\n  length++;\n}\n",
            editable: true,
            instructions: "This loop finds the string's length, by counting characters until it reaches the null character. What should the program state look like after it finishes?",
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "length")) {
                    return "The loop stops when $c{u[length]} is $c{'\\0'}. Which index is that?";
                }
                return ctx.basicHint;
            },
        },
    ],
    next: "98-string-length.html",
    workspace: { allowVariableCreation: true },
});
