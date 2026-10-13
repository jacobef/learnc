import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";
const testInputs = [
    ["12", "18"],
    ["18", "12"],
    ["7", "7"],
    ["1", "9"],
    ["9", "1"],
    ["17", "5"],
    ["14", "15"],
    ["84", "36"],
    ["100", "75"],
    ["48", "180"],
    ["99", "66"],
    ["13", "26"],
    ["81", "27"],
    ["64", "96"],
    ["2", "100"],
    ["91", "65"],
];
createCodeOutputChallengeTemplate({
    inputs: [
        { name: "a", type: "int" },
        { name: "b", type: "int" },
    ],
    outputs: [{ name: "gcd", type: "int" }],
    startInput: ["12", "18"],
    testInputs,
    solve: `int gcd = a;
while (a % gcd != 0 || b % gcd != 0) {
  gcd--;
}`,
    textareaMinLines: 7,
    instructions: "Write code that creates an $t{int} variable named $n{gcd}, whose value is the greatest common divisor of $n{a} and $n{b}. That is, the largest number that both $n{a} and $n{b} are divisible by. For example, if $n{a} is $v{12} and $n{b} is $v{18}, then $n{gcd} should be $v{6}.\n$n{a} and $n{b} will both be positive.",
    hints: (ctx) => {
        if (ctx.behavesLike("int gcd = 1;")) {
            return "Every number is divisible by $v{1}, but $v{1} isn't always the greatest common divisor. If you're counting upward and stopping at the first number that works, try counting downward instead.";
        }
        if (ctx.currentResult.kind === "wrong-output-value") {
            return "One approach: start $n{gcd} at $n{a}, then keep decreasing it until both $n{a} and $n{b} are divisible by it.";
        }
        return null;
    },
    next: "52-prime.html",
});
