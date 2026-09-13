import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";

createCodeOutputChallengeTemplate({
  inputs: [{ name: "n", type: "int" }],
  outputs: [{ name: "slices", type: "int" }],
  startInput: ["10"],
  testInputs: Array.from({ length: 101 }, (_, n) => [String(n)]),
  solve: `int slices = 0;
while (slices < n) {
  slices = slices + 8;
}`,
  textareaMinLines: 6,
  instructions:
    "You have $n{n} friends coming over for a pizza party, and everyone wants a slice. Each pizza has 8 slices.\nYou buy just enough whole pizzas to feed everyone.\nCreate an $t{int} variable named $n{slices} containing the total number of slices you bought. For example, 10 friends would need 2 pizzas, which is $v{16} slices.",
  hints: (ctx) => {
    if (ctx.currentResult.kind === "missing-output") {
      return "You need to create a variable named $n{slices}."
    }
    if (ctx.currentResult.kind === "wrong-output-type") {
      return "$n{slices} should have type $t{int}.";
    }
    if (ctx.currentResult.kind === "step-limit") {
      return "Your program runs forever. Make sure each trip through your $c{while} loop increases $n{slices}. Buying another pizza adds 8 slices.";
    }
    if (ctx.behavesLike(`int slices = 0;
while (slices * 8 < n) {
  slices = slices + 1;
}`)) {
      return "You're counting pizzas, but $n{slices} should count slices. Each pizza has 8 slices.";
    }
    if (ctx.behavesLike(`int slices = 0;
while (slices <= n) {
  slices = slices + 8;
}`)) {
      return "If you already have exactly enough slices, you don't need another pizza.";
    }
    if (ctx.currentResult.ok && !ctx.report.pass) {
      return "That works for the shown number of friends, but not every input. Click $showFailingCaseButton to see one that fails.";
    }
    return null;
  },
  next: "sandbox.html?finished=1",
});
