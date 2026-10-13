import { createCompileQuizTemplate } from "./shared-compile-quiz-template.js";

createCompileQuizTemplate({
  initialInstructions:
    "Compile errors and undefined behavior are different kinds of problems. A compile error is about how a program is written, and the compiler catches it before the program ever runs. Undefined behavior is about what a program does while it runs, so the program compiles fine, and the problem only shows up (or doesn't) when it happens.\nFor each program, decide whether it compiles, doesn't compile, or compiles but has undefined behavior.",
  allowUndefinedAnswer: true,
  steps: [
    {
      code: "int x = 7 / 0;",
      explanation:
        "This compiles. (A real compiler would probably warn you about it, but warnings don't stop a program from compiling.) Dividing by zero is only a problem when it actually happens, while the program is running.",
      hint: "Is dividing by zero a problem with how the program is written, or with what it does when it runs?",
    },
    {
      code: "double d = 4.5;\nint r = d % 2;",
      explanation: "$c{%} only works on integers, and the compiler can tell that $n{d} isn't one.",
    },
    {
      code: "int x;\nint y = x + 1;",
      explanation: "This compiles, but using $n{x}'s value before it has one is undefined behavior.",
    },
    {
      code: "int x = 3;\nint* p = x;",
      explanation: "An $t{int} can't be stored in an $t{int*}. This is about types, which the compiler always knows.",
    },
    {
      code: "int* p;\n*p = 1;",
      explanation:
        "This compiles: $n{p} is an $t{int*}, so $c{*p} is an $t{int}, and assigning $v{1} to it is fine as far as types go. But $n{p} was never given a value, so it doesn't point anywhere, and following it is undefined behavior.",
      hint: "Are the types right? And what does $n{p} point to when line 2 runs?",
    },
    {
      code: "int big = 2147483647;\nint bigger = big + 1;",
      explanation: "This compiles, but $c{big + 1} overflows an $t{int}, which is undefined behavior.",
    },
    {
      code: "int x = 10;\nint y = 0;\nif (y != 0) {\n  x = x / y;\n}",
      explanation: "This is fine: the division only happens if $n{y} isn't zero, and here it is zero, so the division never happens.",
    },
    {
      code: "int x = 3;\nint* p = &x;\ndouble* q = p;",
      explanation: "$n{p} points to an $t{int}, and $n{q} has to point to a $t{double}. Pointers to different types don't mix.",
    },
    {
      code: "int x = 5;\nint y = x / (x - 5);",
      explanation:
        "This compiles, since the compiler doesn't work out what $c{x - 5} will be. But when the program runs, it's $v{0}, so this divides by zero.",
      hint: "What is $c{x - 5}?",
    },
  ],
  next: "42-fix-it.html",
});
