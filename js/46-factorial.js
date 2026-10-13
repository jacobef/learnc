import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";
createCodeOutputChallengeTemplate({
    inputs: [{ name: "n", type: "int" }],
    outputs: [{ name: "factorial", type: "int" }],
    startInput: ["5"],
    testInputs: Array.from({ length: 13 }, (_, n) => [String(n)]),
    solve: `int factorial = 1;
int i = 1;
while (i <= n) {
  factorial *= i;
  i++;
}`,
    textareaMinLines: 8,
    instructions: "Write code that creates an $t{int} variable named $n{factorial}, whose value is $c{1 * 2 * ... * n}. For example, if $n{n} is $v{5}, then $n{factorial} should be $c{1 * 2 * 3 * 4 * 5}, which is $v{120}. If $n{n} is $v{0}, then $n{factorial} should be $v{1}.\nUse a $c{while} loop. $n{n} will be between $v{0} and $v{12}. (Any higher, and the result would overflow an $t{int}.)",
    hints: (ctx) => {
        if (ctx.behavesLike("int factorial = 0;")) {
            return "Your $n{factorial} is always $v{0}. If $n{factorial} starts at $v{0}, multiplying it by anything keeps it at $v{0}.";
        }
        if (ctx.behavesLike(`int factorial = 1;
int i = 1;
while (i < n) {
  factorial *= i;
  i++;
}`)) {
            return "You're multiplying up to $c{n - 1}, but you need to include $n{n} itself.";
        }
        if (ctx.behavesLike(`int factorial = 0;
int i = 1;
while (i <= n) {
  factorial += i;
  i++;
}`)) {
            return "You're adding the numbers together, but you need to multiply them.";
        }
        return null;
    },
    next: "47-for.html",
});
