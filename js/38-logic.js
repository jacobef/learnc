import { createExpressionEvalTemplate } from "./shared-expression-template.js";
function typeHint(box) {
    if (box.type === "")
        return "The type should be $t{int}.";
    if (box.type !== "int") {
        return `$c{!}, $c{&&}, and $c{||} always evaluate to an $t{int}, even when used on $t{double}s. The type should be $t{int}, not $t{${box.type}}.`;
    }
    return null;
}
function valueHint(box) {
    if (box.value === "")
        return "The value shouldn't be empty.";
    if (box.value !== "0" && box.value !== "1") {
        return "$c{!}, $c{&&}, and $c{||} always evaluate to $v{0} or $v{1}.";
    }
    return null;
}
createExpressionEvalTemplate({
    steps: [
        {
            expression: "!0",
            instructions: '$c{!x} evaluates to $v{1} if $c{x} is zero, and $v{0} otherwise. It\'s pronounced "not".',
        },
        {
            expression: "!7",
            instructions: '$c{!x} evaluates to $v{1} if $c{x} is zero, and $v{0} otherwise. It\'s pronounced "not".',
        },
        {
            expression: "!x",
            setup: "double x = 0.5;",
            editable: true,
            fixValueCategory: true,
            hints: (ctx) => {
                const box = ctx.enteredBox;
                if (box.value === "1") {
                    return "$v{0.5} is non-zero.";
                }
                return typeHint(box) ?? valueHint(box) ?? "$v{0.5} is non-zero.";
            },
        },
        {
            expression: "1 && 0",
            instructions: '$c{x && y} evaluates to $v{1} if $c{x} and $c{y} are both non-zero, and $v{0} otherwise. It\'s pronounced "and".',
        },
        {
            expression: "3 && -2",
            editable: true,
            fixValueCategory: true,
            hints: (ctx) => {
                const box = ctx.enteredBox;
                return typeHint(box) ?? valueHint(box) ?? "$v{3} and $v{-2} are both non-zero.";
            },
        },
        {
            expression: "0 || 0",
            instructions: '$c{x || y} evaluates to $v{1} if at least one of $c{x} and $c{y} is non-zero, and $v{0} otherwise. It\'s pronounced "or".\nOn most keyboards, $c{|} is typed with Shift + Backslash.',
        },
        {
            expression: "x || y",
            setup: "int x = 0;\ndouble y = 2.5;",
            editable: true,
            fixValueCategory: true,
            hints: (ctx) => {
                const box = ctx.enteredBox;
                return typeHint(box) ?? valueHint(box) ?? "$n{y}'s value is non-zero.";
            },
        },
        {
            expression: "!x + 1",
            setup: "int x = 3;",
            instructions: "Like negation ($c{-x}) and the other operators that take a single operand, $c{!} has higher precedence than any operator with two sides, so this is parsed as $c{(!x) + 1}, not $c{!(x + 1)}.",
        },
        {
            expression: "0 < x && x < 10",
            setup: "int x = 3;",
            instructions: "$c{&&} and $c{||} have lower precedence than comparisons, so this is parsed as $c{(0 < x) && (x < 10)}. This is the right way to check whether $n{x} is between two numbers. (Recall from $i{28. Clamp} that $c{0 < x < 10} doesn't work.)",
        },
        {
            expression: "!(x > 5) && x != 0",
            setup: "int x = 3;",
            editable: true,
            fixValueCategory: true,
            hints: (ctx) => {
                const box = ctx.enteredBox;
                return typeHint(box) ?? valueHint(box) ?? "This is parsed as $c{(!(x > 5)) && (x != 0)}.";
            },
        },
        {
            expression: "x == 1 || x == 3 && x == 2",
            setup: "int x = 1;",
            editable: true,
            fixValueCategory: true,
            instructions: "$c{&&} has higher precedence than $c{||}.",
            hints: (ctx) => {
                const box = ctx.enteredBox;
                const hint = typeHint(box) ?? valueHint(box);
                if (hint)
                    return hint;
                if (box.value === "0") {
                    return "You're parsing it as $c{(x == 1 || x == 3) && x == 2}, but $c{&&} has higher precedence than $c{||}, so it should be parsed as $c{x == 1 || (x == 3 && x == 2)}.";
                }
                return "This is parsed as $c{(x == 1) || ((x == 3) && (x == 2))}.";
            },
        },
    ],
    next: "39-leap-year.html",
});
