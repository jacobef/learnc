import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";
createCodeOutputChallengeTemplate({
    inputs: [{ name: "a", type: "int[5]" }],
    outputs: [{ name: "sum", type: "int" }],
    testInputs: [
        "{1, 2, 3, 4, 5}", "{0, 0, 0, 0, 0}", "{10, -10, 10, -10, 10}", "{7, 0, 0, 0, 0}",
        "{0, 0, 0, 0, 7}", "{-3, -1, -4, -1, -5}", "{100, 200, 300, 400, 500}", "{2, 7, 1, 8, 2}",
    ].map((a) => [a]),
    startInput: ["{2, 7, 1, 8, 2}"],
    solve: `int sum = 0;
for (int i = 0; i < 5; i++) {
  sum += a[i];
}`,
    textareaMinLines: 6,
    instructions: "Write code that creates an $t{int} variable named $n{sum}, whose value is the sum of $n{a}'s elements.",
    hints: (ctx) => {
        if (ctx.behavesLike(`int sum = 0;
for (int i = 1; i < 5; i++) {
  sum += a[i];
}`)) {
            return "You're skipping $c{a[0]}. Remember that indices start at 0.";
        }
        if (ctx.behavesLike(`int sum = 0;
for (int i = 0; i < 4; i++) {
  sum += a[i];
}`)) {
            return "You're skipping the last element, $c{a[4]}.";
        }
        return null;
    },
    next: "81-count.html",
});
