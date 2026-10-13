import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";
createCodeOutputChallengeTemplate({
    inputs: [{ name: "n", type: "int" }],
    outputs: [{ name: "result", type: "int" }],
    testInputs: Array.from({ length: 11 }, (_, n) => [String(n)]),
    startInput: ["4"],
    main: ([n]) => `int main(void) {\n  int result = sum_to(${n});\n  return 0;\n}`,
    solve: "int sum_to(int n) {\n  if (n == 0) {\n    return 0;\n  }\n  return n + sum_to(n - 1);\n}",
    textareaMinLines: 7,
    instructions: "Write a function named $c{sum_to} that returns $c{1 + 2 + ... + n}, or $v{0} if $n{n} is $v{0}. Don't use a loop: have $c{sum_to} call itself instead.\nThink about how $c{sum_to(n)} relates to $c{sum_to(n - 1)}, and what the base case is.",
    hints: (ctx) => {
        if (ctx.currentResult.kind === "compile" || ctx.currentResult.kind === "ub")
            return null;
        if (ctx.behavesLike("int sum_to(int n) {\n  if (n == 0) {\n    return 1;\n  }\n  return n + sum_to(n - 1);\n}")) {
            return "The base case returns the sum of no numbers at all, which is $v{0}.";
        }
        return "$c{1 + 2 + ... + n} is $c{(1 + 2 + ... + (n - 1)) + n}, which is $c{sum_to(n - 1) + n}.";
    },
    next: "60-pointer-parameters.html",
});
