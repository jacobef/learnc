import { createCompileQuizTemplate } from "./shared-compile-quiz-template.js";

const main = (body: string) => `int main(void) {\n${body}  return 0;\n}`;

createCompileQuizTemplate({
  implicitMain: false,
  allowUndefinedAnswer: true,
  initialInstructions:
    "The compiler checks every function call against the function's declaration: the number of arguments, their types, and how the return value is used. For each program, decide what happens.",
  steps: [
    {
      code: `int add(int x, int y) {\n  return x + y;\n}\n\n${main("  int s = add(1);\n")}`,
      explanation: "$c{add} takes two arguments, so it has to be given two.",
    },
    {
      code: `void reset(int* p) {\n  *p = 0;\n}\n\n${main("  int x = 4;\n  reset(x);\n")}`,
      explanation: "$c{reset} takes an $t{int*}, but $n{x} is an $t{int}. Passing $c{&x} would work.",
    },
    {
      code: `int twice(int n) {\n  return 2 * n;\n}\n\n${main("  int t = twice(twice(3));\n")}`,
      explanation: "This is fine. $c{twice(3)} is an $t{int}, so it can be passed to $c{twice}.",
    },
    {
      code: `void hello(void) {\n}\n\n${main("  int x = hello();\n")}`,
      explanation: "$c{hello} returns $t{void}, which means it doesn't return a value at all, so there's nothing to store in $n{x}.",
    },
    {
      code: `void f(void) {\n  return 3;\n}\n\n${main("  f();\n")}`,
      explanation: "A $t{void} function can use $c{return;} to stop early, but it can't return a value.",
    },
    {
      code: `int square(int n) {\n  return n * n;\n}\n\n${main("  int s = square(3);\n  int t = n;\n")}`,
      explanation: "$n{n} is $c{square}'s parameter, so its name only exists inside $c{square}.",
      hint: "Which function does $n{n} belong to?",
    },
    {
      code: `int sign(int n) {\n  if (n > 0) {\n    return 1;\n  }\n}\n\n${main("  int s = sign(4);\n")}`,
      explanation:
        "This works: $c{sign(4)} returns $v{1}. But $c{sign} has a problem, which the next program runs into.",
    },
    {
      code: `int sign(int n) {\n  if (n > 0) {\n    return 1;\n  }\n}\n\n${main("  int s = sign(-4);\n")}`,
      explanation:
        "When $n{n} isn't positive, $c{sign} reaches its closing brace without returning anything. That compiles, but using the value of a call that didn't return one is undefined behavior.",
      hint: "What does $c{sign(-4)} return?",
    },
    {
      code: `int* make(void) {\n  int x = 3;\n  return &x;\n}\n\n${main("  int* p = make();\n  int y = *p;\n")}`,
      explanation:
        "$n{x} belongs to $c{make}, so it's removed when $c{make} returns. That leaves $n{p} pointing to a variable that no longer exists, and following it is undefined behavior. In a real program, it might even seem to work, until something else reuses that memory.",
      hint: "What happens to a function's variables when it returns?",
    },
  ],
  next: "65-characters.html",
});
