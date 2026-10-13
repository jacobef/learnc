import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";

const testInputs: string[][] = [
  "0", "5", "9", "10", "19", "40", "99", "100", "123", "505", "4096", "9999",
  "10203", "55555", "987654", "1000000", "2147483647",
].map((n) => [n]);

createCodeOutputChallengeTemplate({
  inputs: [{ name: "n", type: "int" }],
  outputs: [{ name: "digit_sum", type: "int" }],
  startInput: ["4096"],
  testInputs,
  solve: `int digit_sum = 0;
while (n > 0) {
  digit_sum += n % 10;
  n /= 10;
}`,
  textareaMinLines: 7,
  instructions:
    "Write code that creates an $t{int} variable named $n{digit_sum}, whose value is the sum of $n{n}'s digits. For example, if $n{n} is $v{4096}, then $n{digit_sum} should be $c{4 + 0 + 9 + 6}, which is $v{19}. $n{n} won't be negative.\nHere's something that might help: $c{n % 10} is $n{n}'s last digit, and $c{n / 10} is $n{n} with its last digit removed. For example, $c{4096 % 10} is $v{6}, and $c{4096 / 10} is $v{409}.\nYou're allowed to change $n{n}'s value.",
  hints: (ctx) => {
    if (ctx.behavesLike("int digit_sum = n % 10;")) {
      return "That's just the last digit. Use a loop to add up all of the digits, one at a time.";
    }
    if (ctx.behavesLike(`int digit_sum = 0;
while (n > 9) {
  digit_sum += n % 10;
  n /= 10;
}`)) {
      return "Your loop stops while there's still one digit left in $n{n}, so the first digit never gets added.";
    }
    return null;
  },
  next: "50-review-iii.html",
});
