import { createExpressionEvalTemplate } from "./shared-expression-template.js";
const setup = "int a[3] = {4, 8, 15};\nint* p = a + 1;";
function selectHint(expression, target, why) {
    return (ctx) => ctx.mode === "selected"
        ? `${why} So $c{${expression}} refers to $c{${target}}.`
        : `$c{${expression}} refers to a variable. Click $fromStateButton and select it.`;
}
createExpressionEvalTemplate({
    initialInstructions: "Here's the strange part. When an array variable is used in almost any expression, it's automatically converted into a pointer to its first element. This is called decay: in $c{a + 1}, $c{a} decays into $c{&a[0]}.\nThat's how $c{a[i]} works. $c{a[i]} means $c{*(a + i)}, just like with a pointer, and $c{a} decays into a pointer to $c{a[0]}. So an array variable is used exactly like a pointer to the first element of an array from $c{malloc}.\nIn $i{84. Indexing}, $c{a} on its own was shown as referring to the whole array. That's what the name itself refers to, but as soon as it's used as part of a larger expression, it decays. (One exception is $c{sizeof a}, which we'll get to.)\nThis is also how $c{p} was set up here: $c{int* p = a + 1;} means $c{int* p = &a[0] + 1;}.",
    steps: [
        {
            expression: "a + 0",
            setup,
            instructions: "$c{a} decays into $c{&a[0]}, so $c{a + 0} is a pointer to $c{a[0]}.",
        },
        {
            expression: "a + 2",
            setup,
            editable: true,
            hints: (ctx) => ctx.mode === "entered"
                ? "$c{a} decays into a pointer to $c{a[0]}, so $c{a + 2} is an $t{int*} pointing to $c{a[2]}."
                : "$c{a + 2} is a pointer value. Click $typeBoxButton.",
        },
        {
            expression: "*a",
            setup,
            editable: true,
            hints: selectHint("*a", "a[0]", "$c{a} decays into a pointer to $c{a[0]}."),
        },
        {
            expression: "*(a + 2)",
            setup,
            editable: true,
            explanation: "This is exactly what $c{a[2]} means: the array decays into a pointer to its first element, the index moves it forward, and $c{*} gives the element.",
            hints: selectHint("*(a + 2)", "a[2]", "$c{a + 2} points to $c{a[2]}."),
        },
        {
            expression: "a[1] + *a",
            setup,
            editable: true,
            hints: (ctx) => ctx.mode === "entered"
                ? "$c{a[1]} is $v{8}, and $c{*a} is $c{a[0]}, which is $v{4}."
                : "This adds two elements' values, so it's a value, not a variable. Click $typeBoxButton.",
        },
    ],
    next: "89-array-sizes.html",
    workspace: { allowUndefinedAnswer: true },
});
