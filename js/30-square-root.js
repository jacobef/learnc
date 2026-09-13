import { createProgramTemplate } from "./shared-program-template.js";
createProgramTemplate({
    initialInstructions: "Another example: This program finds the square root of 25. (Note that it only works because the square root of 25 is a whole number. Otherwise, it would just run forever...)\nNext, you'll write your own program using $c{while}!.",
    steps: [
        { code: "int x = 0;\n" },
        { code: "while (x * x != 25) {\n" },
        { code: "  x = x + 1;\n" },
        { code: "}\n" },
    ],
    workspace: {},
    next: "31-pizza.html",
});
