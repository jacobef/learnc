import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";
const testN = Array.from({ length: 21 }, (_, i) => String(i - 10));
createCodeOutputChallengeTemplate({
    inputs: [{ name: "n", type: "int" }],
    outputs: [{ name: "cubed", type: "int" }],
    startInput: ["4"],
    testInputs: testN.map((value) => [value]),
    solve: "int cubed = n * n * n;",
    textareaMinLines: 6,
    instructions: "Write code that creates an $t{int} variable named $n{cubed}, whose value is $n{n} to the power of 3. It must work for any value of $n{n}.\nThe current test input is shown on line 1. To get a different test input, press $newInputButton.\nWhen you're confident that your code would work for any value of $n{n}, press $checkButton.",
    hints: (ctx) => {
        if (ctx.behavesLike("int cubed = n ^ 3;")) {
            return "In C, $c{^} is not exponentiation; it does something else entirely. (It's called \"bitwise XOR\", and won't help you here.)";
        }
        if (ctx.behavesLike("int cubed = n * n;")) {
            return "You're raising $n{n} to the 2nd power, not the 3rd. You need to multiply by $n{n} once more.";
        }
        if (ctx.behavesLike("int cubed = n * n * n * n;")) {
            return "You're raising $n{n} to the 4th power, not the 3rd. You're multiplying by $n{n} one too many times.";
        }
        if (ctx.currentResult.kind === "missing-output") {
            return "You need to create a variable named $n{cubed}.";
        }
        if (ctx.currentResult.kind === "wrong-output-value") {
            return "$n{cubed} should be $n{n} multiplied by itself three times.";
        }
        if (ctx.currentResult.ok && !ctx.report.pass) {
            return "Your program produces the correct output for the shown value of $n{n}, but fails for at least one other value. Your program must work for any value of $n{n}. Click $showFailingCaseButton to switch to a value of $n{n} where your program doesn't work.";
        }
        return null;
    },
    next: "22-max.html",
    isLast: false,
});
