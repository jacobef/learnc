import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";

const testInputs: string[][] = [
  ["0", "-2", "2"],
  ["-3", "-2", "2"],
  ["3", "-2", "2"],
  ["5.5", "1.25", "4.75"],
  ["1.5", "1.25", "4.75"],
  ["1.0", "1.25", "4.75"],
  ["-6.25", "-5.5", "-1.5"],
  ["-3.0", "-5.5", "-1.5"],
  ["-1.25", "-5.5", "-1.5"],
  ["7.5", "7.5", "7.5"],
  ["2.25", "2.25", "8.5"],
  ["8.5", "2.25", "8.5"],
  ["-0.5", "-0.5", "0.5"],
  ["0.5", "-0.5", "0.5"],
  ["0.75", "-0.5", "0.5"],
];

createCodeOutputChallengeTemplate({
  inputs: [
    { name: "n", type: "double" },
    { name: "low", type: "double" },
    { name: "high", type: "double" },
  ],
  outputs: [{ name: "clamped", type: "double" }],
  startInput: ["9.25", "2.0", "6.5"],
  testInputs,
  solve: `double clamped = n;
if (clamped < low) {
  clamped = low;
}
if (clamped > high) {
  clamped = high;
}`,
  textareaMinLines: 9,
  instructions:
    'Write code that creates a $t{double} variable named $n{clamped}. It should "clamp" $n{n} between $n{low} and $n{high}. That is:\nIf $n{n} is already between $n{low} and $n{high}, then $n{clamped} should equal $n{n}. If $n{n} is below $n{low}, then $n{clamped} should equal $n{low}. If $n{n} is above $n{high}, then $n{clamped} should equal $n{high}.',
  hints: (ctx) => {
    if (
      ctx.behavesLike(`double clamped = n;
if (clamped < low) {
  clamped = low;
}`)
    ) {
      return "You handled values below $n{low}, but not values above $n{high}.";
    }
    if (
      ctx.behavesLike(`double clamped = n;
if (clamped > high) {
  clamped = high;
}`)
    ) {
      return "You handled values above $n{high}, but not values below $n{low}.";
    }
    if (
      ctx.behavesLike(`double clamped = n;
if (clamped < low) {
  clamped = high;
} else if (clamped > high) {
  clamped = low;
}`)
    ) {
      return "The bounds are swapped. Values below $n{low} should clamp to $n{low}, and values above $n{high} should clamp to $n{high}.";
    }
    if (ctx.currentResult.kind === "missing-output") {
      return "Create an output variable named $n{clamped}.";
    }
    if (ctx.currentResult.kind === "wrong-output-type") {
      return "$n{clamped} should have type $t{double}.";
    }
    if (ctx.currentResult.kind === "wrong-output-value") {
      return "$n{clamped} should stay as $n{n} when $n{n} is inside the range, otherwise it should be set to the nearest bound.";
    }
    if (ctx.currentResult.ok && !ctx.report.pass) {
      return "The shown input works, but at least one other test input fails. Click $showFailingCaseButton to jump to one that fails.";
    }
    return null;
  },
  next: "25-review-ii.html",
  isLast: false,
});
