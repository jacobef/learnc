import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";

createCodeOutputChallengeTemplate({
  inputs: [
    { name: "s", type: "char[]" },
    { name: "letter", type: "char" },
  ],
  outputs: [{ name: "count", type: "int" }],
  testInputs: [
    ['"banana"', "'a'"],
    ['"banana"', "'n'"],
    ['"banana"', "'z'"],
    ['""', "'a'"],
    ['"aaaa"', "'a'"],
    ['"Mississippi"', "'s'"],
    ['"Mississippi"', "'M'"],
    ['"Mississippi"', "'m'"],
    ['"a b c d"', "' '"],
  ],
  startInput: ['"banana"', "'a'"],
  solve: `int count = 0;
for (int i = 0; s[i] != '\\0'; i++) {
  if (s[i] == letter) {
    count++;
  }
}`,
  textareaMinLines: 8,
  instructions:
    "Write code that creates an $t{int} variable named $n{count}, whose value is the number of times $n{letter} appears in the string $n{s}. For example, if $n{s} is $c{\"banana\"} and $n{letter} is $c{'a'}, then $n{count} should be $v{3}.\nA $c{for} loop's condition doesn't have to compare the loop variable with a number. Any condition works.",
  hints: (ctx) => {
    if (ctx.currentResult.kind === "wrong-output-value" && ctx.currentCase.inputLiterals[1] === "'m'") {
      return "Uppercase and lowercase letters have different codes, so $c{'M'} doesn't count as an $c{'m'}.";
    }
    return null;
  },
  next: "100-string-literals.html",
});
