import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";

const testInputs: string[][] = [
  ["-10.75"],
  ["-3.5"],
  ["-2.5"],
  ["-2.25"],
  ["-1.5"],
  ["-1.25"],
  ["-0.75"],
  ["-0.5"],
  ["-0.25"],
  ["0.0"],
  ["0.25"],
  ["0.5"],
  ["1.25"],
  ["1.5"],
  ["2.5"],
  ["7.75"],
];

createCodeOutputChallengeTemplate({
  inputs: [{ name: "n", type: "double" }],
  outputs: [{ name: "rounded", type: "int" }],
  startInput: ["-2.5"],
  testInputs,
  solve: `int rounded;
if (n >= 0) {
  rounded = n + 0.5;
} else {
  rounded = n - 0.5;
}`,
  textareaMinLines: 9,
  instructions:
    "Write code that creates an $t{int} variable named $n{rounded}, which is $n{n} rounded to the nearest integer. This level includes both positive and negative inputs.\n",
  hints: (ctx) => {
    if (
      ctx.behavesLike(`int rounded = n;
if (rounded > n) {
  rounded = rounded - 1;
}`)
    ) {
      return "This always rounds down. For nearest-integer rounding, values should sometimes round up instead.";
    }
    if (
      ctx.behavesLike(`int rounded = n;
if (rounded < n) {
  rounded = rounded + 1;
}`)
    ) {
      return "This always rounds up. For nearest-integer rounding, values should sometimes round down instead.";
    }
    if (ctx.behavesLike("int rounded = n + 0.5;")) {
      return "That works for non-negative values, but fails for negative ones. Handle the positive and negative cases separately.";
    }
    if (ctx.behavesLike("int rounded = n - 0.5;")) {
      return "That works for negative values, but fails for non-negative ones. Handle the positive and negative cases separately.";
    }
    if (ctx.behavesLike("int rounded = n;")) {
      return "That drops the decimal part instead of rounding to the nearest integer.";
    }
    if (ctx.currentResult.kind === "missing-output") {
      return "You need to create a variable named $n{rounded}.";
    }
    if (ctx.currentResult.kind === "wrong-output-type") {
      return "$n{rounded} should have type $t{int}.";
    }
    if (ctx.currentResult.ok && !ctx.report.pass) {
      return "The shown input works, but another tested input fails. Click $showFailingCaseButton to jump to one.";
    }
    return null;
  },
  next: "28-while.html",
  isLast: false,
});
