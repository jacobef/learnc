import { createCompileQuizTemplate } from "./shared-compile-quiz-template.js";

createCompileQuizTemplate({
  initialInstructions:
    "Many compile errors come from mixing up types. The compiler knows every variable's type, and it checks that each value is used in a way that makes sense for its type. Pointers are an especially common source of these errors.\nFor each program, decide whether it compiles.",
  steps: [
    {
      code: "int x = 3;\nint* p = x;",
      explanation:
        "An $t{int} isn't an address, so it can't be stored in an $t{int*}. To make $n{p} point to $n{x}, write $c{&x}.",
      hint: "What type is $n{x}, and what type is $n{p}?",
    },
    {
      code: "int x = 3;\nint* p = &x;\n*p = *p;",
      explanation: "This one is fine. It doesn't do anything useful, but each side of $c{=} is an $t{int}.",
    },
    {
      code: "int x = 3;\nint* p = &x;\nint y = p;",
      explanation:
        "$n{p} is an address, and $n{y} holds an $t{int}. To store the value $n{p} points to, write $c{*p}.",
    },
    {
      code: "int x = 3;\nint y = *x;",
      explanation: "$c{*} follows a pointer to the variable it points to. $n{x} is an $t{int}, not a pointer, so there's nothing to follow.",
    },
    {
      code: "int* p = &5;",
      explanation: "$c{&} gives the address of a variable. $v{5} is just a number, so it doesn't have an address.",
    },
    {
      code: "int x = 1;\n&x = 0;",
      explanation:
        "$c{&x} is the address of $n{x}: a value, like $v{5}. A variable's address can't be changed.",
      hint: "Is $c{&x} something that can hold a value?",
    },
    {
      code: "int* p = &y;\nint y = 2;",
      explanation: "$n{y} has to be declared before its address can be taken.",
    },
    {
      code: "int x = 3;\nint* p = &x;\nint** q = &p;\n**q = 4;",
      explanation: "This compiles: $c{**q} follows $n{q} to $n{p}, and then $n{p} to $n{x}.",
    },
    {
      code: "int x = 3;\nint* p = &x;\nint** q = p;",
      explanation:
        "$n{q} has to point to an $t{int*}, but $n{p} points to an $t{int}. Pointers to different types don't mix. Writing $c{&p} would work.",
      hint: "What does $n{p} point to, and what does $n{q} need to point to?",
    },
  ],
  next: "13-whitespace-comments.html",
});
