import { createProgramTemplate } from "./shared-program-template.js";
const basic = (ctx) => ctx.basicHint;
createProgramTemplate({
    implicitMain: false,
    initialInstructions: "Another preprocessor directive, $c{#define}, defines a macro: a name that the preprocessor replaces with some other text, everywhere it appears after the definition. The compiler never even sees the name, only what it's replaced with.\nMacro names are usually written in all capitals, so it's obvious which names are macros.\nClick $runLineButton to continue.",
    steps: [
        { code: "#define LIMIT 3\n" },
        { code: "#define SQUARE(x) ((x) * (x))\n" },
        { code: "#define BAD_SQUARE(x) x * x\n\n" },
        { code: "int main(void) {\n" },
        {
            code: "  int a = LIMIT * 2;\n",
            instructions: "Before compiling, $c{LIMIT} was replaced by $c{3}, so this line was compiled as $c{int a = 3 * 2;}. Macros like this are often used to name a constant used throughout a program, so it can be changed in one place.",
        },
        {
            code: "  int b = SQUARE(a + 1);\n",
            editable: true,
            instructions: "A macro can take arguments, like a function, but it's still just text replacement: $c{SQUARE(a + 1)} becomes $c{((a + 1) * (a + 1))}.",
            hints: basic,
        },
        {
            code: "  int c = BAD_SQUARE(a + 1);\n",
            editable: true,
            instructions: "That's why $c{SQUARE} has so many parentheses. Without them, $c{BAD_SQUARE(a + 1)} becomes $c{a + 1 * a + 1}. What does that evaluate to?",
            hints: (ctx) => ctx.basicHintTopicIs("value", "c")
                ? "$c{*} has higher precedence than $c{+}, so $c{a + 1 * a + 1} is $c{a + (1 * a) + 1}."
                : ctx.basicHint,
        },
        {
            code: "  return 0;\n",
            instructions: "Macros are also how header files avoid being included twice. A header usually starts with $c{#ifndef POWERS_H} (\"if $c{POWERS_H} isn't defined\") and $c{#define POWERS_H}, and ends with $c{#endif}. The second time it's included, $c{POWERS_H} is already defined, so the preprocessor skips everything up to the $c{#endif}. This is called an include guard.",
        },
        { code: "}\n" },
    ],
    next: "75-sizes.html",
    workspace: { allowVariableCreation: true, allowVariableDeletion: true },
});
