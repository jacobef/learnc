import { createProgramTemplate } from "./shared-program-template.js";
createProgramTemplate({
    initialInstructions: "A computer's memory is made of bytes, and each byte has its own address. A variable takes up several bytes in a row, and its address is the address of its first byte. That's why the addresses of $t{int} variables have always been at least 4 apart: each $t{int} takes up 4 bytes.\nThe $c{sizeof} operator gives the number of bytes a type, or a variable, takes up.\nClick $runLineButton to continue.",
    steps: [
        { code: "int a = sizeof(int);\n" },
        {
            code: "int b = sizeof(double);\n",
            editable: true,
            instructions: "How many bytes does a $t{double} take up? If you're not sure, look at the addresses of the $t{double} variables in earlier lessons.",
            hints: (ctx) => ctx.basicHintTopicIs("value", "b") ? "A $t{double} takes up 8 bytes." : ctx.basicHint,
        },
        {
            code: "int c = sizeof(char);\n",
            instructions: "A $t{char} is exactly one byte, on every computer. The other sizes can be different on different computers, but the ones here are the most common.",
        },
        { code: "int* p = &a;\n" },
        {
            code: "int d = sizeof(int*);\n",
            instructions: "A pointer is an address, and on this computer, an address takes up 8 bytes, whatever type it points to.",
        },
        {
            code: "int e = sizeof a;\n",
            editable: true,
            instructions: "$c{sizeof} can also be given a variable, in which case it gives the size of the variable's type. (The parentheses are only needed around a type.)",
            hints: (ctx) => ctx.basicHintTopicIs("value", "e") ? "$n{a} is an $t{int}." : ctx.basicHint,
        },
        { code: "double x = 1.5;\n" },
        {
            code: "int f = sizeof(x * 2);\n",
            editable: true,
            instructions: "$c{sizeof} can be given any expression. It gives the size of the expression's type, without even evaluating it.",
            hints: (ctx) => ctx.basicHintTopicIs("value", "f") ? "$c{x * 2} is a $t{double}." : ctx.basicHint,
        },
    ],
    next: "76-arrays-i.html",
});
