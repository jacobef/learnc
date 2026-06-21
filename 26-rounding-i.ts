import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";

const testInputs: string[][] = [
  ["0"],
  ["0.125"],
  ["0.25"],
  ["0.5"],
  ["0.625"],
  ["1.25"],
  ["1.5"],
  ["1.75"],
  ["2.0"],
  ["2.5"],
  ["3.5"],
  ["10.75"],
  ["15.5"],
  ["31.375"],
];

createCodeOutputChallengeTemplate({
  inputs: [{ name: "n", type: "double" }],
  outputs: [{ name: "rounded", type: "int" }],
  startInput: ["2.5"],
  testInputs,
  solve: "int rounded = n + 0.5;",
  textareaMinLines: 6,
  instructions:
    "Write code that creates an $t{int} variable named $n{rounded}, which is $n{n} rounded to the nearest integer. In this level, you can assume $n{n} will not be negative.\nRecall that assigning a $t{double} to an $t{int} drops the decimal part, which provides a means of rounding down.",
  hints: (ctx) => {
    if (ctx.behavesLike("int rounded = n;")) {
      return "You're always rounding down. Values like $v{2.75} should become $v{3}, not $v{2}.";
    }
    if (ctx.behavesLike("int rounded = n + 1;")) {
      return "You're always rounding up. Values like $v{1.25} should become $v{1}, not $v{2}.";
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
  next: "27-rounding-ii.html",
  isLast: false,
});
