import { createCompileQuizTemplate } from "./shared-compile-quiz-template.js";
createCompileQuizTemplate({
    initialInstructions: "Blocks and $c{if} statements bring a few new kinds of compile errors. For each program, decide whether it compiles.",
    steps: [
        {
            code: "int x = 1;\n{\n  int y = 2;\n}\nx = y;",
            explanation: "$n{y} only exists inside the block it's declared in. After the block's closing brace, its name can't be used.",
            hint: "Where does $n{y} stop existing?",
        },
        {
            code: "int x = 5;\n{\n  int x = 6;\n}",
            explanation: "This one is fine! A variable in an inner block can have the same name as one outside it. Inside the block, the inner $n{x} hides the outer one; this is called shadowing. It's legal, but usually confusing, so avoid it.",
            hint: "Are the two $n{x}s in the same block?",
        },
        {
            code: "int x = 5;\nif x > 3 {\n  x = 0;\n}",
            explanation: "An $c{if} statement's condition has to be in parentheses: $c{if (x > 3)}.",
        },
        {
            code: "int x = 5;\nelse {\n  x = 1;\n}",
            explanation: "An $c{else} has to come right after an $c{if}'s block. On its own, it doesn't mean anything.",
        },
        {
            code: "int x = 0;\nif (x > 0) {\n  int y = 1;\n} else {\n  y = 2;\n}",
            explanation: "The $c{if} block and the $c{else} block are separate blocks. $n{y} is declared in the first one, so it doesn't exist in the second.",
            hint: "Which block is $n{y} declared in?",
        },
        {
            code: "int x = 5;\nif (x > 3)\n  x = 0;\nelse {\n  x = 1;\n}",
            explanation: "This compiles. An $c{if} or $c{else} can be followed by a single statement instead of a block. It's best to always use braces anyway: without them, it's easy to add a second line that looks like it's inside the $c{if}, but isn't.",
        },
        {
            code: "int x = 2;\nif (x = 3) {\n  x = 0;\n}",
            explanation: "This compiles, but it's almost certainly a mistake. $c{x = 3} is an assignment, not a comparison: it sets $n{x} to $v{3}, and its value is $v{3}, which counts as true. Comparing uses $c{==}. (Real compilers usually warn about this.)",
            hint: "Is $c{=} the same as $c{==}?",
        },
        {
            code: "double d = 4.5;\nint* p = &d;",
            explanation: "$c{&d} points to a $t{double}, but $n{p} has to point to an $t{int}.",
        },
    ],
    next: "24-abs.html",
});
