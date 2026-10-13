import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";

createCodeOutputChallengeTemplate({
  inputs: [
    { name: "a", type: "int[6]" },
    { name: "target", type: "int" },
  ],
  outputs: [{ name: "count", type: "int" }],
  testInputs: [
    ["{1, 2, 3, 2, 1, 2}", "2"],
    ["{1, 2, 3, 2, 1, 2}", "4"],
    ["{5, 5, 5, 5, 5, 5}", "5"],
    ["{0, 0, 1, 0, 0, 0}", "1"],
    ["{9, 0, 0, 0, 0, 9}", "9"],
    ["{-1, 1, -1, 1, -1, 1}", "-1"],
    ["{4, 8, 15, 16, 23, 42}", "42"],
    ["{4, 8, 15, 16, 23, 42}", "4"],
    ["{7, 7, 7, 0, 0, 7}", "7"],
  ],
  startInput: ["{1, 2, 3, 2, 1, 2}", "2"],
  solve: `int count = 0;
for (int i = 0; i < 6; i++) {
  if (a[i] == target) {
    count++;
  }
}`,
  textareaMinLines: 8,
  instructions:
    "Write code that creates an $t{int} variable named $n{count}, whose value is the number of elements of $n{a} that are equal to $n{target}. For example, if $n{a} is $c{\\{1, 2, 3, 2, 1, 2\\}} and $n{target} is $v{2}, then $n{count} should be $v{3}.",
  hints: (ctx) => {
    if (ctx.behavesLike(`int count = 0;
for (int i = 0; i < 6; i++) {
  if (a[i] == target) {
    count = i;
  }
}`)) {
      return "$n{count} should be the number of matching elements, not the index of a matching element.";
    }
    return null;
  },
  next: "82-reverse.html",
});
