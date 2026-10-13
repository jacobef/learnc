import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";
const lists = [
    "{1, 2, 3, 4, 5}", "{0, 0, 0}", "{10, -10, 10, -10}", "{7}", "{}",
    "{0, 0, 0, 0, 7}", "{-3, -1, -4, -1, -5, -9}", "{100, 200, 300}", "{2, 7, 1, 8, 2, 8}",
];
const length = (list) => (list === "{}" ? 0 : list.split(",").length);
createCodeOutputChallengeTemplate({
    inputs: [
        { name: "a", type: "int*", allocated: true },
        { name: "n", type: "int" },
    ],
    outputs: [{ name: "sum", type: "int" }],
    testInputs: lists.map((list) => [list, String(length(list))]),
    startInput: ["{2, 7, 1, 8, 2}", "5"],
    solve: `int sum = 0;
for (int i = 0; i < n; i++) {
  sum += a[i];
}`,
    textareaMinLines: 6,
    instructions: "$n{a} points to the first element of an array of $n{n} $t{int}s. Write code that creates an $t{int} variable named $n{sum}, whose value is the sum of the array's elements. If $n{n} is $v{0}, $n{sum} should be $v{0}.",
    hints: (ctx) => {
        if (ctx.behavesLike(`int sum = 0;
for (int i = 1; i < n; i++) {
  sum += a[i];
}`)) {
            return "You're skipping $c{a[0]}. Remember that indices start at 0.";
        }
        if (ctx.behavesLike(`int sum = 0;
for (int i = 0; i < n - 1; i++) {
  sum += a[i];
}`)) {
            return "You're skipping the last element, $c{a[n - 1]}.";
        }
        return null;
    },
    next: "81-count.html",
});
