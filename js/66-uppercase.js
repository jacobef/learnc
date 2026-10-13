import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";
createCodeOutputChallengeTemplate({
    inputs: [{ name: "c", type: "char" }],
    outputs: [{ name: "upper", type: "char" }],
    testInputs: ["'a'", "'m'", "'z'", "'A'", "'Q'", "'Z'", "'0'", "'9'", "' '", "'!'", "'`'", "'{'", "'@'", "'['"].map((c) => [c]),
    startInput: ["'m'"],
    solve: `char upper = c;
if (c >= 'a' && c <= 'z') {
  upper = c - 'a' + 'A';
}`,
    textareaMinLines: 6,
    instructions: "Write code that creates a $t{char} variable named $n{upper}. If $n{c} is a lowercase letter, $n{upper} should be the same letter in uppercase. Otherwise, $n{upper} should be the same as $n{c}.\nFor example, if $n{c} is $c{'m'}, then $n{upper} should be $c{'M'}; and if $n{c} is $c{'7'}, then $n{upper} should be $c{'7'}.",
    hints: (ctx) => {
        if (ctx.behavesLike("char upper = c - 'a' + 'A';")) {
            return "That works for lowercase letters, but anything else should stay unchanged. Check whether $n{c} is between $c{'a'} and $c{'z'} first.";
        }
        if (ctx.behavesLike("char upper = c - 32;")) {
            return "That works for lowercase letters, but anything else should stay unchanged. Check whether $n{c} is between $c{'a'} and $c{'z'} first.";
        }
        return null;
    },
    next: "67-putchar.html",
});
