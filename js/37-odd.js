import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";
createCodeOutputChallengeTemplate({
    inputs: [{ name: "n", type: "int" }],
    outputs: [{ name: "odd", type: "int" }],
    startInput: ["7"],
    testInputs: Array.from({ length: 21 }, (_, i) => [String(i - 10)]),
    solve: "int odd = n % 2 != 0;",
    textareaMinLines: 6,
    instructions: "Write code that creates an $t{int} variable named $n{odd}, whose value is $v{1} if $n{n} is odd, and $v{0} if $n{n} is even.",
    hints: (ctx) => {
        if (ctx.behavesLike("int odd = n % 2 == 1;")) {
            return "Recall from $i{36. Remainder} that if the left side of $c{%} is negative, the remainder is negative or zero. For example, $c{-3 % 2} is $v{-1}, not $v{1}.";
        }
        if (ctx.behavesLike("int odd = n % 2;")) {
            return "That works for positive numbers, but recall from $i{36. Remainder} that if the left side of $c{%} is negative, the remainder is negative or zero. For example, $c{-3 % 2} is $v{-1}, but $n{odd} should be $v{1}.";
        }
        if (ctx.behavesLike("int odd = n % 2 == 0;")) {
            return "That's $v{1} when $n{n} is even, but it should be $v{1} when $n{n} is odd.";
        }
        if (ctx.currentResult.kind === "wrong-output-value") {
            return "A number is even if it's divisible by 2, and odd otherwise.";
        }
        return null;
    },
    next: "38-logic.html",
});
