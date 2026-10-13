import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";

createCodeOutputChallengeTemplate({
  inputs: [{ name: "a", type: "int[5]" }],
  outputs: [{ name: "a", type: "int[5]" }],
  testInputs: [
    "{1, 2, 3, 4, 5}", "{5, 4, 3, 2, 1}", "{0, 0, 0, 0, 0}", "{7, 0, 0, 0, 0}",
    "{1, 2, 1, 2, 1}", "{-3, 8, 0, 11, 6}", "{42, 42, 7, 42, 42}",
  ].map((a) => [a]),
  startInput: ["{1, 2, 3, 4, 5}"],
  solve: `for (int i = 0; i < 2; i++) {
  int saved = a[i];
  a[i] = a[4 - i];
  a[4 - i] = saved;
}`,
  textareaMinLines: 8,
  instructions:
    "Write code that reverses the order of $n{a}'s elements. For example, if $n{a} starts as $c{\\{1, 2, 3, 4, 5\\}}, it should end up as $c{\\{5, 4, 3, 2, 1\\}}.\nThis time, the result is $n{a} itself, not a new variable. You can create other variables if you need to.",
  hints: (ctx) => {
    if (ctx.behavesLike(`for (int i = 0; i < 5; i++) {
  a[i] = a[4 - i];
}`)) {
      return "Once $c{a[0]} is overwritten with $c{a[4]}'s value, $c{a[0]}'s original value is gone, so it can't be copied to $c{a[4]}. Save it in another variable first.";
    }
    if (ctx.behavesLike(`for (int i = 0; i < 5; i++) {
  int saved = a[i];
  a[i] = a[4 - i];
  a[4 - i] = saved;
}`)) {
      return "Each pair of elements gets swapped twice: once on the way to the middle, and again on the way back. Only loop over the first half of the array.";
    }
    if (ctx.currentResult.kind === "wrong-output-value") {
      return "Try swapping $c{a[0]} with $c{a[4]}, then $c{a[1]} with $c{a[3]}. ($c{a[2]} stays where it is.)";
    }
    return null;
  },
  next: "83-pointers-to-elements.html",
});
