import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";

createCodeOutputChallengeTemplate({
  inputs: [{ name: "n", type: "int" }],
  outputs: [{ name: "prime", type: "int" }],
  startInput: ["9"],
  testInputs: Array.from({ length: 61 }, (_, n) => [String(n)]),
  solve: `int prime = n >= 2;
for (int d = 2; d < n; d++) {
  if (n % d == 0) {
    prime = 0;
  }
}`,
  textareaMinLines: 9,
  instructions:
    "Write code that creates an $t{int} variable named $n{prime}, whose value is $v{1} if $n{n} is prime, and $v{0} otherwise.\nA number is prime if it's at least 2, and it isn't divisible by any number other than 1 and itself. For example, 7 is prime, but 9 isn't, since it's divisible by 3. $n{n} won't be negative.",
  hints: (ctx) => {
    if (ctx.behavesLike("int prime = n % 2 != 0;")) {
      return "Not every odd number is prime. For example, 9 is divisible by 3.";
    }
    if (ctx.behavesLike("int prime = 0;")) {
      return "Your $n{prime} is always $v{0}. Every number is divisible by $v{1} and by itself, so make sure you're only checking the numbers in between.";
    }
    if (ctx.behavesLike(`int prime = 1;
for (int d = 2; d < n; d++) {
  if (n % d == 0) {
    prime = 0;
  }
}`)) {
      return "Almost! But $v{0} and $v{1} aren't prime.";
    }
    if (ctx.currentResult.kind === "wrong-output-value") {
      return "Try every number between 2 and $c{n - 1}, and check whether $n{n} is divisible by it.";
    }
    return null;
  },
  next: "53-collatz.html",
});
