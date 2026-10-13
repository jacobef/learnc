import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";
const testInputs = [
    ["2", "0"],
    ["2", "1"],
    ["2", "10"],
    ["2", "30"],
    ["3", "4"],
    ["5", "3"],
    ["7", "2"],
    ["10", "5"],
    ["-2", "3"],
    ["-3", "2"],
    ["-1", "7"],
    ["0", "3"],
    ["1", "20"],
    ["9", "0"],
];
createCodeOutputChallengeTemplate({
    inputs: [
        { name: "base", type: "int" },
        { name: "exponent", type: "int" },
    ],
    outputs: [{ name: "power", type: "int" }],
    startInput: ["3", "4"],
    testInputs,
    solve: `int power = 1;
for (int i = 0; i < exponent; i++) {
  power *= base;
}`,
    textareaMinLines: 6,
    instructions: "Write code that creates an $t{int} variable named $n{power}, whose value is $n{base} to the power of $n{exponent}. For example, if $n{base} is $v{3} and $n{exponent} is $v{4}, then $n{power} should be $c{3 * 3 * 3 * 3}, which is $v{81}. Anything to the power of $v{0} is $v{1}.\nUse a $c{for} loop. $n{exponent} won't be negative.",
    hints: (ctx) => {
        if (ctx.behavesLike("int power = base ^ exponent;")) {
            return "Recall from $i{25. Cubed} that $c{^} is not exponentiation in C.";
        }
        if (ctx.behavesLike("int power = 0;")) {
            return "Your $n{power} is always $v{0}. If $n{power} starts at $v{0}, multiplying it by anything keeps it at $v{0}.";
        }
        if (ctx.behavesLike(`int power = 1;
for (int i = 0; i <= exponent; i++) {
  power *= base;
}`)) {
            return "You're multiplying by $n{base} one too many times.";
        }
        if (ctx.behavesLike(`int power = 1;
for (int i = 1; i < exponent; i++) {
  power *= base;
}`)) {
            return "You're multiplying by $n{base} one too few times.";
        }
        return null;
    },
    next: "49-digit-sum.html",
});
