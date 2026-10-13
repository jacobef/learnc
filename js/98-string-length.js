import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";
createCodeOutputChallengeTemplate({
    inputs: [{ name: "s", type: "char[]" }],
    outputs: [{ name: "length", type: "int" }],
    testInputs: ['""', '"a"', '"hi"', '"hello"', '"C Boxes"', '"     "', '"0123456789"', '"abc\\0def"'].map((s) => [s]),
    startInput: ['"hello"'],
    solve: `int length = 0;
while (s[length] != '\\0') {
  length++;
}`,
    textareaMinLines: 6,
    instructions: "Write code that creates an $t{int} variable named $n{length}, whose value is the length of the string in $n{s}: the number of characters before the null character. For example, if $n{s} is $c{\"hello\"}, then $n{length} should be $v{5}.\nTry to write it without looking back at the previous lesson.",
    hints: (ctx) => {
        if (ctx.behavesLike(`int length = 0;
while (s[length] != '\\0') {
  length++;
}
length++;`)) {
            return "You're counting the null character too. The length is the number of characters before it.";
        }
        if (ctx.behavesLike(`int length = 0;
while (s[length] != '0') {
  length++;
}`)) {
            return "$c{'0'} is the digit 0 (code $v{48}). The null character is $c{'\\0'} (code $v{0}).";
        }
        return null;
    },
    next: "99-count-letter.html",
});
