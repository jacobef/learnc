import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";

createCodeOutputChallengeTemplate({
  inputs: [{ name: "n", type: "int" }],
  outputs: [{ name: "sum", type: "int" }],
  startInput: ["4"],
  testInputs: Array.from({ length: 31 }, (_, n) => [String(n)]),
  solve: `int sum = 0;
int i = 1;
while (i <= n) {
  sum += i;
  i++;
}`,
  textareaMinLines: 8,
  instructions:
    "Write code that creates an $t{int} variable named $n{sum}, whose value is $c{1 + 2 + ... + n}. For example, if $n{n} is $v{4}, then $n{sum} should be $c{1 + 2 + 3 + 4}, which is $v{10}. If $n{n} is $v{0}, then $n{sum} should be $v{0}.\nUse a $c{while} loop. You'll need another variable that counts from $v{1} up to $n{n}.",
  hints: (ctx) => {
    if (ctx.behavesLike(`int sum = 0;
int i = 1;
while (i < n) {
  sum += i;
  i++;
}`)) {
      return "You're adding up to $c{n - 1}, but you need to include $n{n} itself.";
    }
    if (ctx.behavesLike(`int sum = 0;
int i = 1;
while (i <= n) {
  sum = i;
  i++;
}`)) {
      return "$c{sum = i;} replaces $n{sum}'s value. You want to add $n{i} to $n{sum}.";
    }
    return null;
  },
  next: "46-factorial.html",
});
