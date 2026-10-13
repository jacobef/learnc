import { createCompileQuizTemplate } from "./shared-compile-quiz-template.js";
const main = (body) => `int main(void) {\n${body}  return 0;\n}`;
createCompileQuizTemplate({
    implicitMain: false,
    allowUndefinedAnswer: true,
    initialInstructions: "Functions and arrays have some rules of their own. For each program, decide what happens.",
    steps: [
        {
            code: `int numbers(void)[3];\n\n${main("")}`,
            explanation: "This declares $c{numbers} as a function returning an array of 3 $t{int}s, which C doesn't allow: a function can't return an array. (It can return a pointer, or fill in an array it's given a pointer to.)",
            hint: "Read the declaration the usual way: $c{numbers(...)[i]} is an $t{int}. So what would $c{numbers} return?",
        },
        {
            code: `void f(int a[3]) {\n  a = a + 1;\n}\n\n${main("  int b[3];\n  f(b);\n")}`,
            explanation: "In a parameter list, $c{int a[3]} means $c{int *a}. Even with the 3 written in, $n{a} is an ordinary pointer, so it can be assigned to.",
            hint: "What does $c{int a[3]} mean in a parameter list?",
        },
        {
            code: `void f(int a[3]) {\n}\n\n${main("  int b[5];\n  f(b);\n")}`,
            explanation: "The 3 is ignored: $c{f} takes a pointer to an $t{int}, and $c{b} decays into one, whatever $n{b}'s length is.",
        },
        {
            code: `void f(int a[3]) {\n}\n\n${main("  int x = 4;\n  f(&x);\n")}`,
            explanation: "This compiles too, since $c{&x} is a pointer to an $t{int}. (Whether $c{f} can safely use $c{a[1]} is another matter.)",
        },
        {
            code: `void f(int a[3]) {\n}\n\n${main("  f({1, 2, 3});\n")}`,
            explanation: "An initializer list isn't an expression, so it can't be passed as an argument.",
        },
        {
            code: `void set(int a[]) {\n  a[0] = 5;\n}\n\n${main("  int b[2] = {1, 2};\n  set(b);\n")}`,
            explanation: "This works, and changes $c{b[0]}: $n{a} points to $n{b}'s first element, so $c{a[0]} is $c{b[0]}. This is how functions change arrays they're given.",
        },
        {
            code: `int *make(void) {\n  int a[3] = {1, 2, 3};\n  return a;\n}\n\n${main("  int *p = make();\n  int x = p[0];\n")}`,
            explanation: "$n{a} belongs to $c{make}, so it's removed when $c{make} returns. That leaves $n{p} pointing to an array that no longer exists. Using a pointer like that is undefined behavior; in a real program, it might even seem to work, until something else reuses that memory.",
            hint: "What happens to a function's variables when it returns?",
        },
    ],
    next: "97-strings.html",
});
