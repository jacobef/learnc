import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";

createCodeOutputChallengeTemplate({
  inputs: [{ name: "n", type: "int" }],
  outputs: [{ name: "pencils", type: "int" }],
  startInput: ["10"],
  testInputs: Array.from({ length: 101 }, (_, n) => [String(n)]),
  solve: `int pencils = 0;
while (pencils < n) {
  pencils = pencils + 6;
}`,
  textareaMinLines: 6,
  instructions:
    "Pencils come in packs of 6. What’s the smallest number of pencils you can buy if you need at least $n{n} pencils?\nPut the answer in an $t{int} variable named $n{pencils}.\nExample: if $n{n} is $v{9}, then $n{pencils} should be $v{12}.",
  hints: (ctx) => {
    if (ctx.currentResult.kind === "missing-output") {
      return "You need to create a variable named $n{pencils}.";
    }
    if (ctx.currentResult.kind === "wrong-output-type") {
      return "$n{pencils} should have type $t{int}.";
    }
    if (ctx.currentResult.kind === "step-limit") {
      return "Your loop hasn't finished. Check that each trip through it increases $n{pencils}. Buying another pack adds 6 pencils.";
    }
    if (ctx.behavesLike(`int pencils = 0;
while (pencils * 6 < n) {
  pencils = pencils + 1;
}`)) {
      return "You're counting packs, but $n{pencils} should count pencils. Each pack has 6 pencils.";
    }
    if (ctx.behavesLike(`int pencils = 0;
while (pencils <= n) {
  pencils = pencils + 6;
}`)) {
      return "If you already have exactly enough pencils, you don't need another pack.";
    }
    if (ctx.currentResult.ok && !ctx.report.pass) {
      return "That works for the shown input, but not every input. Click $showFailingCaseButton to see one that fails.";
    }
    return null;
  },
  next: "sandbox.html?finished=1",
});
