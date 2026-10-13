import { createExpressionEvalTemplate } from "./shared-expression-template.js";
const xy = "int x = 7;\nint y = 0;";
const big = "int big = 2147483647;";
createExpressionEvalTemplate({
    initialInstructions: "Some operations aren't allowed in C, like dividing by zero. If a program does one anyway, the result is called undefined behavior: the C language makes no promises at all about what happens next. On a real computer, the program might crash, or keep running with a garbage value, or do something stranger, and it might do something different every time it runs. In this tutorial, undefined behavior just stops the program with an error.\nFor each expression, either enter its type and value, or press $undefinedButton if evaluating it would be undefined behavior.",
    steps: [
        {
            expression: "7 / 0",
            instructions: "Dividing by zero is undefined behavior.",
        },
        {
            expression: "x % y",
            setup: xy,
            editable: true,
            fixValueCategory: true,
            explanation: "$c{x % y} is the remainder when $n{x} is divided by $n{y}, and $n{y} is $v{0}. Taking a remainder by zero is just as undefined as dividing by zero.",
            hints: (ctx) => {
                if (ctx.mode !== "undefined") {
                    return "$c{x % y} is the remainder when $n{x} is divided by $n{y}. What is $n{y}'s value?";
                }
                return null;
            },
        },
        {
            expression: "y / x",
            setup: xy,
            editable: true,
            fixValueCategory: true,
            explanation: "Dividing zero by something is fine. Only dividing by zero is undefined.",
            hints: (ctx) => {
                if (ctx.mode === "undefined") {
                    return "This divides $v{0} by $v{7}, not $v{7} by $v{0}.";
                }
                return "$c{y / x} is $v{0} divided by $v{7}.";
            },
        },
        {
            expression: "u + 1",
            setup: "int u;",
            instructions: "Using a variable's value before it's been given one is also undefined behavior.",
        },
        {
            expression: "u * 0",
            setup: "int u;",
            editable: true,
            fixValueCategory: true,
            explanation: "Anything times $v{0} is $v{0}, but that doesn't help here: calculating $c{u * 0} still uses $n{u}'s value, and $n{u} doesn't have one.",
            hints: (ctx) => {
                if (ctx.mode !== "undefined") {
                    return "It's true that anything times $v{0} is $v{0}, but this still uses $n{u}'s value, which it doesn't have yet.";
                }
                return null;
            },
        },
        {
            expression: "*p",
            setup: "int a = 3;\nint* p;",
            editable: true,
            explanation: "$c{*p} refers to the variable whose address is $n{p}'s value. But $n{p} doesn't have a value yet, so using it is undefined behavior. Notice that $n{p} doesn't point to $n{a} just because $n{a} happens to be the only other variable.",
            hints: (ctx) => {
                if (ctx.mode !== "undefined") {
                    return "$c{*p} refers to the variable whose address is $n{p}'s value. What is $n{p}'s value?";
                }
                return null;
            },
        },
        {
            expression: "big + 1",
            setup: big,
            instructions: "An $t{int} can only hold whole numbers from $v{-2147483648} to $v{2147483647}. (You don't need to memorize these; they're in the $referenceButton, under \"Ranges\".) A calculation whose result goes outside its type's range is called an overflow, and overflowing an $t{int} is undefined behavior.",
        },
        {
            expression: "big - 1",
            setup: big,
            editable: true,
            fixValueCategory: true,
            explanation: "$v{2147483646} is inside the range of an $t{int}, so this is fine.",
            hints: (ctx) => {
                if (ctx.mode === "undefined") {
                    return "Is $v{2147483646} inside the range of an $t{int}?";
                }
                return "What is $v{2147483647} minus $v{1}?";
            },
        },
        {
            expression: "big * 2 / 2",
            setup: big,
            editable: true,
            fixValueCategory: true,
            explanation: "In math, multiplying by 2 and then dividing by 2 would just give back $n{big}. But C evaluates one operation at a time: first $c{big * 2}, then the division. $c{big * 2} would be $v{4294967294}, which doesn't fit in an $t{int}, so the expression has already overflowed before the division happens. If any single step of a calculation overflows, the whole thing is undefined, even if later steps would have brought it back into range.",
            hints: (ctx) => {
                if (ctx.mode !== "undefined") {
                    return "This is parsed as $c{(big * 2) / 2}, so $c{big * 2} is calculated first. What would $c{big * 2} be?";
                }
                return null;
            },
        },
        {
            expression: "-big - 1",
            setup: big,
            editable: true,
            fixValueCategory: true,
            explanation: "The range of an $t{int} isn't symmetric: it goes one further in the negative direction than in the positive direction. $v{-2147483648} is exactly the smallest possible $t{int}, so this is fine, but subtracting 1 more would overflow.",
            hints: (ctx) => {
                if (ctx.mode === "undefined") {
                    return "Look at the range of an $t{int} again. What's the smallest value it can hold?";
                }
                return "$c{-big} is $v{-2147483647}. What's one less than that?";
            },
        },
    ],
    next: "41-errors-or-ub.html",
    workspace: { allowUndefinedAnswer: true },
});
