import { createExpressionEvalTemplate } from "./shared-expression-template.js";
const setup = "int x = 7;\nint y = -2;\nint a = 10;\nint b = 20;";
const valueHint = (why) => (ctx) => ctx.mode === "entered" ? why : "Click $typeBoxButton.";
createExpressionEvalTemplate({
    initialInstructions: "The conditional operator, $c{c ? x : y}, is like an $c{if}-$c{else} inside an expression: if $c{c} is non-zero, it's $c{x}, and otherwise it's $c{y}. Only the chosen side is evaluated. It's the only operator with three operands, so it's sometimes called the ternary operator.\nFor each expression, enter its type and value, or select the variable it refers to.",
    steps: [
        {
            expression: "x > 0 ? 1 : -1",
            setup,
            instructions: "$c{x > 0} is true, so this is $v{1}.",
        },
        {
            expression: "y > 0 ? 1 : -1",
            setup,
            editable: true,
            fixValueCategory: true,
            hints: valueHint("$c{y > 0} is false, so this is the right side."),
        },
        {
            expression: "x > y ? x : y",
            setup,
            editable: true,
            fixValueCategory: true,
            explanation: "This is a common way to write the larger of two values.",
            hints: valueHint("$c{x > y} is true, so this is $n{x}'s value."),
        },
        {
            expression: "x % 2 == 0 ? x / 2 : 3 * x + 1",
            setup,
            editable: true,
            fixValueCategory: true,
            explanation: "The conditional operator has very low precedence: lower than everything except assignment, so the condition and both sides don't need parentheses.",
            hints: valueHint("$n{x} is odd, so this is $c{3 * x + 1}."),
        },
        {
            expression: "y != 0 ? x / y : 0",
            setup,
            editable: true,
            fixValueCategory: true,
            hints: valueHint("$n{y} isn't $v{0}, so this is $c{x / y}, with the fraction dropped."),
        },
        {
            expression: "y == 0 ? 0 : x / 0",
            setup,
            editable: true,
            fixValueCategory: true,
            explanation: "$n{y} isn't zero, so the right side is the one that's evaluated, and it divides by zero.",
            hints: (ctx) => (ctx.mode === "undefined" ? null : "Which side gets evaluated?"),
        },
    ],
    next: "121-bitwise.html",
    workspace: { allowUndefinedAnswer: true },
});
