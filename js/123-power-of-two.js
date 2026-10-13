import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";
createCodeOutputChallengeTemplate({
    inputs: [{ name: "n", type: "unsigned int" }],
    outputs: [{ name: "bits", type: "int" }],
    testInputs: ["0", "1", "2", "3", "7", "8", "12", "255", "256", "1023", "65536", "4294967295", "2147483648"].map((n) => [`${n}u`]),
    startInput: ["12u"],
    solve: "int bits = 0;\nwhile (n != 0) {\n  bits += n & 1;\n  n >>= 1;\n}",
    textareaMinLines: 6,
    instructions: "Write code that creates an $t{int} variable named $n{bits}, whose value is the number of 1 bits in $n{n}'s binary form. For example, $v{12} is $c{1100} in binary, so if $n{n} is $v{12}, $n{bits} should be $v{2}.\n$c{n & 1} is $n{n}'s last bit, and $c{n >> 1} removes it, just like $c{n % 10} and $c{n / 10} for decimal digits. Compound assignment works with bitwise operators too: $c{n >>= 1;}.",
    hints: (ctx) => ctx.currentResult.kind === "wrong-output-value"
        ? "Add up $c{n & 1} for each bit, shifting $n{n} right by one each time, until $n{n} is $v{0}."
        : null,
    next: "124-the-machine.html",
});
