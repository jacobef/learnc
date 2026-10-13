import { createExpressionEvalTemplate } from "./shared-expression-template.js";
const setup = "int a[4] = {10, 20, 30, 40};\nint i = 1;";
function selectHint(expression, target, why) {
    return (ctx) => {
        if (ctx.mode !== "selected") {
            return `$c{${expression}} refers to one of the array's elements. Click $fromStateButton and select it.`;
        }
        if (ctx.selectedName === "a") {
            return `$c{${expression}} refers to a single element, not the whole array. Click the element itself.`;
        }
        return `${why} So $c{${expression}} is $c{${target}}.`;
    };
}
createExpressionEvalTemplate({
    initialInstructions: "For each expression, either select the variable it refers to (by clicking $fromStateButton), enter its type and value (by clicking $typeBoxButton), or click $undefinedButton.\nYou can select a single element of an array by clicking it.",
    steps: [
        {
            expression: "a[2]",
            setup,
            instructions: "$c{a[2]} refers to the element of $n{a} with index 2.",
        },
        {
            expression: "a[i]",
            setup,
            editable: true,
            hints: selectHint("a[i]", "a[1]", "$n{i}'s value is $v{1}."),
        },
        {
            expression: "a[i + 2]",
            setup,
            editable: true,
            hints: selectHint("a[i + 2]", "a[3]", "$c{i + 2} is $v{3}."),
        },
        {
            expression: "a[a[0] / 10 - 1]",
            setup,
            editable: true,
            hints: selectHint("a[a[0] / 10 - 1]", "a[0]", "$c{a[0]} is $v{10}, so the index is $c{10 / 10 - 1}, which is $v{0}."),
        },
        {
            expression: "a[3] - a[i]",
            setup,
            editable: true,
            hints: (ctx) => ctx.mode === "entered"
                ? "$c{a[3]} is $v{40} and $c{a[i]} is $c{a[1]}."
                : "This is a subtraction, so it evaluates to a value, not a variable. Click $typeBoxButton.",
        },
        {
            expression: "a[4]",
            setup,
            editable: true,
            explanation: "$n{a} has 4 elements, so its indices only go from 0 to 3. Using an index outside an array is undefined behavior. It's a very common mistake, so watch out for it!",
            hints: (ctx) => ctx.mode === "undefined"
                ? null
                : "How many elements does $n{a} have? Remember that indices start at 0.",
        },
        {
            expression: "a[i - 2]",
            setup,
            editable: true,
            explanation: "$c{i - 2} is $v{-1}. Negative indices are outside the array too.",
            hints: (ctx) => ctx.mode === "undefined" ? null : "What is $c{i - 2}? Is that a valid index?",
        },
        {
            expression: "a",
            setup,
            instructions: "On its own, $c{a} refers to the whole array.",
        },
    ],
    next: "79-array-loops.html",
    workspace: { allowUndefinedAnswer: true },
});
