import { createProgramTemplate } from "./shared-program-template.js";
// Declaration and use trees are drawn side by side; matching nodes share a color,
// and each node of a use is labeled with its type.
const node = (text, pair, note, child) => ({
    text,
    kind: child ? "operator" : "name",
    pair,
    note,
    children: child ? [child] : undefined,
});
const createHint = (variable, type, reading) => (ctx) => {
    if (ctx.basicHintTopicIs("type", variable) || (ctx.boxNamed(variable) && !ctx.basicHint)) {
        return `${reading} Remove the name from the declaration to get the type: $t{${type}}.`;
    }
    return ctx.basicHint;
};
createProgramTemplate({
    initialInstructions: "When a declarator has more than one operator, which one applies first? The same as in an expression: $c{[]} has higher precedence than $c{*}, so in the expression $c{*q[0]}, the $c{[0]} applies to $n{q} first, and the $c{*} applies to the result. It's $c{*(q[0])}, not $c{(*q)[0]}.\nClick $runLineButton to continue.",
    initialDiagrams: [{ title: "The expression *q[0]", tree: node("*", 1, undefined, node("[0]", 2, undefined, node("q", 3))) }],
    steps: [
        { code: "int a[3] = {1, 2, 3};\n" },
        {
            code: "int *q[3];\n",
            editable: true,
            instructions: "So this declarator is grouped the same way, and so is a use of $n{q}: $c{*q[i]} means $c{*(q[i])}, and the declaration says it's an $t{int}. Working down the use tree: since $c{*q[i]} is an $t{int}, $c{q[i]} is a pointer to an $t{int}, and so $n{q} is an array of 3 pointers to $t{int}s. Its type is $t{int *[3]}. Create $n{q}.\n(For types like this, you can click the ? next to the type in the program state to see it spelled out.)",
            diagrams: [
                { title: "Declaration: int *q[3];", tree: node("*", 1, undefined, node("[3]", 2, undefined, node("q", 3))) },
                { title: "Use: *q[i]", tree: node("*", 1, "int", node("[i]", 2, "int *", node("q", 3, "int *[3]"))) },
            ],
            hints: createHint("q", "int *[3]", "$n{q} is an array of 3 pointers to $t{int}s."),
        },
        {
            code: "q[0] = &a[1];\n",
            editable: true,
            hints: (ctx) => ctx.basicHint,
        },
        {
            code: "*q[0] = 9;\n",
            editable: true,
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "q[0]")) {
                    return "$c{*q[0]} means $c{*(q[0])}, which changes the variable $c{q[0]} points to, not $c{q[0]} itself. Click $resetButton and try again.";
                }
                return ctx.basicHint;
            },
        },
        {
            code: "int (*r)[3];\n",
            editable: true,
            instructions: "As in expressions, parentheses change the grouping. Now the $c{*} applies to $n{r} first. Working down the use tree: $c{(*r)[i]} is an $t{int}, so $c{*r} is an array of 3 $t{int}s, so $n{r} is a pointer to an array of 3 $t{int}s. What's its type? (The parentheses stay in the type: $t{int (*)[3]}.)",
            diagrams: [
                { title: "Declaration: int (*r)[3];", tree: node("[3]", 1, undefined, node("*", 2, undefined, node("r", 3))) },
                { title: "Use: (*r)[i]", tree: node("[i]", 1, "int", node("*", 2, "int[3]", node("r", 3, "int (*)[3]"))) },
            ],
            hints: createHint("r", "int (*)[3]", "$n{r} is a pointer to an array of 3 $t{int}s."),
        },
        {
            code: "r = &a;\n",
            editable: true,
            instructions: "$c{&a} is a pointer to the whole array $n{a}. (This is the other place where an array doesn't decay: when it's the operand of $c{&}.) It has the same address as $c{&a[0]}, but a different type: $t{int (*)[3]} instead of $t{int *}.",
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "r")) {
                    return "$c{&a} is $n{a}'s address, which is the same as its first element's address.";
                }
                return ctx.basicHint;
            },
        },
        {
            code: "(*r)[2] = 5;\n",
            editable: true,
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value")) {
                    return "$c{*r} is the array $n{a}, so $c{(*r)[2]} is $c{a[2]}.";
                }
                return ctx.basicHint;
            },
        },
    ],
    next: "93-declaration-practice.html",
    workspace: { allowVariableCreation: true },
});
