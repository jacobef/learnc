import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";

createCodeOutputChallengeTemplate({
  inputs: [{ name: "n", type: "int" }],
  outputs: [{ name: "steps", type: "int" }],
  startInput: ["6"],
  testInputs: Array.from({ length: 30 }, (_, i) => [String(i + 1)]),
  solve: `int steps = 0;
while (n != 1) {
  if (n % 2 == 0) {
    n /= 2;
  } else {
    n = 3 * n + 1;
  }
  steps++;
}`,
  textareaMinLines: 10,
  instructions:
    "Start with $n{n}. If it's even, divide it by 2. If it's odd, multiply it by 3 and add 1. Repeat until you reach 1. For example, starting from 6, you'd go 6 → 3 → 10 → 5 → 16 → 8 → 4 → 2 → 1.\nWrite code that creates an $t{int} variable named $n{steps}, whose value is the number of steps it takes to reach 1. For example, if $n{n} is $v{6}, then $n{steps} should be $v{8}. If $n{n} is $v{1}, then $n{steps} should be $v{0}.\nNobody has ever proven that this always reaches 1, but it does for every number that's been tried. This is called the Collatz conjecture.",
  hints: (ctx) => {
    if (ctx.behavesLike(`int steps = 1;
while (n != 1) {
  if (n % 2 == 0) {
    n /= 2;
  } else {
    n = 3 * n + 1;
  }
  steps++;
}`)) {
      return "You're counting one step too many. If $n{n} is already $v{1}, $n{steps} should be $v{0}.";
    }
    return null;
  },
  next: "54-functions-i.html",
});
