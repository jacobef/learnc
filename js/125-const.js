import { createCompileQuizTemplate } from "./shared-compile-quiz-template.js";
const main = (body) => `int main(void) {\n${body}  return 0;\n}`;
createCompileQuizTemplate({
    implicitMain: false,
    initialInstructions: "$c{const} in a declaration means a value can't be changed after it's initialized; trying is a compile error. With pointers, where the $c{const} goes matters, and it follows the same rules as the rest of a declaration: $c{const int *p} says $c{*p} is a $c{const int}, so the pointed-to $t{int} can't be changed through $n{p}. $c{int *const p} says $n{p} itself is $c{const}, so $n{p} can't be pointed elsewhere.\nFor each program, decide whether it compiles.",
    steps: [
        {
            code: main("  const int limit = 10;\n  limit = 20;\n"),
            explanation: "$n{limit} is $c{const}, so it can't be assigned to.",
        },
        {
            code: main("  const int limit = 10;\n  int x = limit * 2;\n"),
            explanation: "Reading a $c{const} variable is fine.",
        },
        {
            code: main("  int x = 1;\n  const int *p = &x;\n  *p = 2;\n"),
            explanation: "$c{*p} is a $c{const int}, so it can't be changed through $n{p}, even though $n{x} itself isn't $c{const}.",
        },
        {
            code: main("  int x = 1;\n  int y = 2;\n  const int *p = &x;\n  p = &y;\n"),
            explanation: "This is fine: $n{p} itself isn't $c{const}, only what it points to. It can be pointed somewhere else.",
        },
        {
            code: main("  int x = 1;\n  int y = 2;\n  int *const p = &x;\n  p = &y;\n"),
            explanation: "Now $n{p} is $c{const}, so it can't be pointed elsewhere.",
        },
        {
            code: main("  int x = 1;\n  int *const p = &x;\n  *p = 2;\n"),
            explanation: "This is fine: $n{p} is $c{const}, but $c{*p} isn't.",
        },
        {
            code: `int length(const char *s) {\n  int n = 0;\n  while (s[n] != '\\0') {\n    n++;\n  }\n  return n;\n}\n\n${main('  char word[] = "hi";\n  int n = length(word);\n')}`,
            explanation: "This is fine, and it's what $c{const} is usually for: a parameter like $c{const char *s} promises that the function won't change the characters, like $c{puts}'s parameter. A $t{char *} can be passed to it; it just can't be used to change anything.",
        },
        {
            code: `void clear(const char *s) {\n  s[0] = '\\0';\n}\n\n${main('  char word[] = "hi";\n  clear(word);\n')}`,
            explanation: "$c{clear} promised not to change the characters, so it can't.",
        },
    ],
    next: "126-structs-i.html",
});
