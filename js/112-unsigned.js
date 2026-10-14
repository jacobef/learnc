import { createProgramTemplate } from "./shared-program-template.js";
const basic = (ctx) => ctx.basicHint;
createProgramTemplate({
    implicitMain: false,
    initialInstructions: "Each integer type also comes in an $c{unsigned} version, which can't hold negative numbers, but can hold positive numbers twice as large. An $t{unsigned int} holds values from $v{0} to $v{4294967295}.\nThe types so far are signed (and $c{signed int} is another way to write $t{int}). Except for $t{char}: whether a plain $t{char} is signed depends on the computer, so $c{signed char} and $c{unsigned char} exist when it matters.\nClick $runLineButton to continue.",
    steps: [
        { code: "int main(void) {\n" },
        { code: "  unsigned int u = 0;\n" },
        {
            code: "  u = u - 1;\n",
            instructions: "Unlike signed overflow, unsigned overflow is not undefined behavior: unsigned arithmetic wraps around. Going below $v{0} wraps around to the largest value, and going past the largest value wraps around to $v{0}, like a car's odometer.",
        },
        {
            code: "  u = u + 2;\n",
            editable: true,
            hints: basic,
        },
        {
            code: "  unsigned char c = 300;\n",
            editable: true,
            instructions: "Converting to an unsigned type always wraps around too. An $t{unsigned char} holds $v{0} to $v{255}, so values wrap around every $v{256}.",
            hints: (ctx) => (ctx.basicHintTopicIs("value", "c") ? "$c{300 - 256} is $v{44}." : ctx.basicHint),
        },
        { code: "  int negative = -1;\n" },
        {
            code: "  unsigned int converted = negative;\n",
            editable: true,
            hints: (ctx) => ctx.basicHintTopicIs("value", "converted")
                ? "$v{-1} is one below $v{0}, so it wraps around to the largest $t{unsigned int}."
                : ctx.basicHint,
        },
        {
            code: "  int less = negative < 1u;\n",
            instructions: "Here's the catch. $c{1u} is an $t{unsigned int}, since the suffix $c{u} makes a number unsigned. When a calculation mixes an $t{int} and an $t{unsigned int}, the $t{int} is converted to $t{unsigned int} first, so this compares $v{4294967295} with $v{1}. Mixing signed and unsigned values is a common source of bugs.",
        },
        { code: "  return 0;\n" },
        { code: "}\n" },
    ],
    next: "113-conversions.html",
    workspace: { allowVariableCreation: true, allowVariableDeletion: true },
});
