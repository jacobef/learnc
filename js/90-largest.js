import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";
const lists = [
    "{3, 9, 2}", "{7}", "{-5, -2, -9}", "{1, 2, 3, 4, 5, 6, 7, 8}", "{8, 7, 6, 5, 4, 3, 2, 1}",
    "{0, 0, 0, 0}", "{4, 4, 9, 9, 1}", "{-1, 100, -1}", "{2147483647, 0}",
];
createCodeOutputChallengeTemplate({
    inputs: [
        { name: "a", type: "int*", allocated: true },
        { name: "n", type: "int" },
    ],
    outputs: [{ name: "largest", type: "int" }],
    testInputs: lists.map((list) => [list, String(list.split(",").length)]),
    startInput: ["{3, 9, 2}", "3"],
    solve: `int largest = a[0];
for (int i = 1; i < n; i++) {
  if (a[i] > largest) {
    largest = a[i];
  }
}`,
    textareaMinLines: 7,
    instructions: "$n{a} points to the first element of an array from $c{malloc}, which has $n{n} $t{int}s, where $n{n} is at least $v{1}. The program doesn't know $n{n} in advance, which is why it couldn't just declare an array. Write code that creates an $t{int} variable named $n{largest}, whose value is the largest element.",
    hints: (ctx) => {
        if (ctx.behavesLike(`int largest = 0;
for (int i = 0; i < n; i++) {
  if (a[i] > largest) {
    largest = a[i];
  }
}`)) {
            return "Starting $n{largest} at $v{0} goes wrong when every element is negative. Start it at the first element instead.";
        }
        return ctx.currentResult.kind === "ub"
            ? "Make sure every index you use is from $v{0} to $c{n - 1}."
            : null;
    },
    next: "91-declarations.html",
});
