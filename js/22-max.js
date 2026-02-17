import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";
const testInputs = [
    ["0", "0"],
    ["1", "2"],
    ["2", "1"],
    ["-1.5", "4.25"],
    ["4.25", "-1.5"],
    ["-5.5", "-2.25"],
    ["-2.25", "-5.5"],
    ["9.125", "9.125"],
    ["-10.75", "10.5"],
    ["10.5", "-10.75"],
    ["7.0", "3.5"],
    ["3.5", "7.0"],
    ["-8.0", "-8.0"],
    ["1234.5", "1234.375"],
    ["-999.875", "-1000.125"],
];
createCodeOutputChallengeTemplate({
    inputs: [
        { name: "x", type: "double" },
        { name: "y", type: "double" },
    ],
    outputs: [{ name: "max", type: "double" }],
    startInput: ["-8.5", "7.25"],
    testInputs,
    solve: `double max = x;
if (y > x) {
  max = y;
}`,
    textareaMinLines: 7,
    instructions: "Write code that creates a $t{double} variable named $n{max}, whose value is the larger of $n{x} and $n{y}.\nLook at $i{20. Absolute Value} again if you're stuck.\nIf you want an extra hard challenge, don't use $c{if}.",
    hints: (ctx) => {
        if (ctx.behavesLike(`double max = x;
if (y < x) {
  max = y;
}`)) {
            return "That computes the smaller value. $n{max} should be the larger of $n{x} and $n{y}.";
        }
        if (ctx.currentResult.kind === "missing-output") {
            return "Create an output variable named $n{max}.";
        }
        if (ctx.currentResult.kind === "wrong-output-type") {
            return "$n{max} should have type $t{double}.";
        }
        if (ctx.currentResult.kind === "wrong-output-value") {
            return "$n{max} should be the larger of $n{x} and $n{y}.";
        }
        if (ctx.currentResult.ok && !ctx.report.pass) {
            return "The shown input works, but some other tested input fails. Click $showFailingCaseButton to jump to one that fails.";
        }
        return null;
    },
    next: "23-else-if.html",
    isLast: false,
});
