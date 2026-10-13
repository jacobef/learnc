import { createCompileQuizTemplate } from "./shared-compile-quiz-template.js";

createCompileQuizTemplate({
  initialInstructions:
    "Before a C program runs, a program called a compiler translates it into instructions the computer can run, checking it for mistakes along the way. If the compiler finds a mistake, it reports a compile error, and the program doesn't run at all. You've probably already seen a few of these while writing code.\nFor each program, decide whether it compiles. If it doesn't, you'll see the compiler's message and the line it points to.",
  steps: [
    {
      code: "int x = 5\nint y = 6;",
      explanation: "Line 1 is missing its semicolon. Every statement ends with one.",
      hint: "Look closely at the end of each line.",
    },
    {
      code: "int x;\nx = 4;",
      explanation: "This one is fine: $n{x} is declared, then given a value.",
    },
    {
      code: "int x = 5;\ny = x;",
      explanation: "$n{y} was never declared. A variable has to be declared before it can be used.",
      hint: "Where is $n{y} created?",
    },
    {
      code: "x = 3;\nint x;",
      explanation:
        "The compiler reads a program from top to bottom, so on line 1, $n{x} hasn't been declared yet. Declaring it later doesn't help.",
      hint: "Which line declares $n{x}, and which line uses it?",
    },
    {
      code: "int x = 5;\nint x = 6;",
      explanation:
        "$n{x} can only be declared once. To change its value, assign to it instead: $c{x = 6;}.",
    },
    {
      code: "Int x = 3;",
      explanation: "C cares about capitalization. The type is $t{int}, not $c{Int}.",
    },
    {
      code: "int 2x = 5;",
      explanation:
        "Names can contain letters, digits, and underscores, but they can't start with a digit.",
      hint: "What is this variable's name?",
    },
    {
      code: "int x2 = 5;",
      explanation: "This one is fine: a name can contain digits, as long as it doesn't start with one.",
    },
    {
      code: "int x = 3;\n5 = x;",
      explanation:
        "The left side of $c{=} has to be something that can hold a value, like a variable. $v{5} is just a number, so nothing can be stored in it.",
      hint: "Which side of $c{=} gets changed?",
    },
    {
      code: "int x = 3;\nint y = 4;\nx = y;\ny = x;",
      explanation:
        "This compiles. (It doesn't swap $n{x} and $n{y}, though: by the time line 4 runs, $n{x} is already $v{4}.) A program can compile and still not do what you meant.",
    },
  ],
  next: "7-fix-it-i.html",
});
