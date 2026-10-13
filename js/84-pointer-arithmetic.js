import { createExpressionEvalTemplate } from "./shared-expression-template.js";
const setup = "int a[4] = {10, 20, 30, 40};\nint* p = &a[1];";
function selectHint(expression, target) {
    return (ctx) => ctx.mode === "selected"
        ? `$n{p} points to $c{a[1]}, so $c{${expression}} refers to $c{${target}}.`
        : `$c{${expression}} refers to a variable. Click $fromStateButton and select it.`;
}
createExpressionEvalTemplate({
    initialInstructions: "Adding an integer to a pointer to an array element gives a pointer to a different element of the same array. $c{p + 1} points to the element after the one $n{p} points to, $c{p + 2} to the one after that, and so on. Subtracting goes backwards. This is called pointer arithmetic.\nThe result has to stay inside the array, or point just past its last element; going any further is undefined behavior, even without using the pointer.\nFor each expression, select the variable it refers to, enter its type and value, or click $undefinedButton.",
    steps: [
        {
            expression: "p + 1",
            setup,
            instructions: "$c{p + 1} points to $c{a[2]}. Notice that its value is $n{p}'s value plus 4, not plus 1: adding 1 to a pointer moves it forward by one element, and each $t{int} takes up 4 bytes.",
        },
        {
            expression: "*(p + 1)",
            setup,
            instructions: "So $c{*(p + 1)} refers to $c{a[2]}.",
        },
        {
            expression: "*(p + 2)",
            setup,
            editable: true,
            hints: selectHint("*(p + 2)", "a[3]"),
        },
        {
            expression: "*(p - 1)",
            setup,
            editable: true,
            hints: selectHint("*(p - 1)", "a[0]"),
        },
        {
            expression: "p + 2",
            setup,
            editable: true,
            hints: (ctx) => ctx.mode === "entered"
                ? "$c{p + 2} is a pointer, so its type is $t{int*}. Its value is the address of the element 2 after $c{a[1]}."
                : "$c{p + 2} doesn't refer to a variable; it's a pointer value. Click $typeBoxButton.",
        },
        {
            expression: "*p + 1",
            setup,
            editable: true,
            explanation: "Like $c{-}, $c{*} has higher precedence than $c{+}, so this is $c{(*p) + 1}, which adds 1 to $n{*p}'s value.",
            hints: (ctx) => ctx.mode === "entered"
                ? "$c{*} has higher precedence than $c{+}, so this is $c{(*p) + 1}."
                : "Is this $c{*(p + 1)} or $c{(*p) + 1}? $c{*} has higher precedence than $c{+}.",
        },
        {
            expression: "*(p + 3)",
            setup,
            editable: true,
            explanation: "$c{p + 3} points just past the end of $n{a}. A pointer like that is allowed to exist, but there's no element there, so using $c{*} on it is undefined behavior, just like $c{a[4]}.",
            hints: (ctx) => ctx.mode === "undefined" ? null : "$n{p} points to $c{a[1]}. Which element would be 3 after that?",
        },
        {
            expression: "p - 2",
            setup,
            editable: true,
            explanation: "$c{p - 2} would point before $n{a}'s first element. Just computing a pointer like that is undefined behavior, even if it's never used.",
            hints: (ctx) => ctx.mode === "undefined" ? null : "$n{p} points to $c{a[1]}. What's 2 elements before it?",
        },
    ],
    next: "85-decay.html",
    workspace: { allowUndefinedAnswer: true },
});
