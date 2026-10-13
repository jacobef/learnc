import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";
createCodeOutputChallengeTemplate({
    inputs: [
        { name: "a", type: "int*", allocated: true },
        { name: "n", type: "int" },
        { name: "target", type: "int" },
    ],
    outputs: [{ name: "count", type: "int" }],
    testInputs: [
        ["{1, 2, 3, 2, 1, 2}", "6", "2"],
        ["{1, 2, 3, 2, 1, 2}", "6", "4"],
        ["{5, 5, 5, 5, 5, 5}", "6", "5"],
        ["{0, 0, 1, 0, 0, 0}", "6", "1"],
        ["{9, 0, 0, 0, 0, 9}", "6", "9"],
        ["{-1, 1, -1, 1, -1, 1}", "6", "-1"],
        ["{4, 8, 15, 16, 23, 42}", "6", "42"],
        ["{4, 8, 15, 16, 23, 42}", "6", "4"],
        ["{7, 7}", "2", "7"],
        ["{3}", "1", "4"],
        ["{2, 0, 2, 0, 2, 0, 2, 0, 2}", "9", "2"],
    ],
    startInput: ["{1, 2, 3, 2, 1, 2}", "6", "2"],
    solve: `int count = 0;
for (int i = 0; i < n; i++) {
  if (a[i] == target) {
    count++;
  }
}`,
    textareaMinLines: 8,
    instructions: "$n{a} points to the first element of an array of $n{n} $t{int}s. Write code that creates an $t{int} variable named $n{count}, whose value is the number of elements that are equal to $n{target}. For example, if the elements are $v{1, 2, 3, 2, 1, 2} and $n{target} is $v{2}, then $n{count} should be $v{3}.",
    hints: (ctx) => {
        if (ctx.behavesLike(`int count = 0;
for (int i = 0; i < n; i++) {
  if (a[i] == target) {
    count = i;
  }
}`)) {
            return "$n{count} should be the number of matching elements, not the index of a matching element.";
        }
        return null;
    },
    next: "82-arrays-i.html",
});
