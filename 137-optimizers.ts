import { createCompileQuizTemplate } from "./shared-compile-quiz-template.js";

createCompileQuizTemplate({
  implicitMain: false,
  initialInstructions:
    "There's more to it. Compilers don't just translate each line: they optimize, transforming the program into faster instructions that give the same results, like keeping variables in registers, or working out answers while compiling. But \"the same results\" only has to be true for programs without undefined behavior. So an optimizer is allowed to assume that undefined behavior never happens, and real ones rely on that all the time, because it lets them make code much faster. When a program does have undefined behavior, that assumption is wrong, and the optimized program can do things that don't correspond to any line of the original.\nFor each program, decide what an optimizing compiler is allowed to do.",
  steps: [
    {
      code: "int will_overflow(int x) {\n  return x + 1 < x;\n}",
      choices: [
        "When x is INT_MAX, it has to return 1, since x + 1 wraps around to a negative number.",
        "It can return 0 every time, without doing any arithmetic.",
        "It has to check whether x is INT_MAX.",
      ],
      answer: 1,
      explanation:
        "If $c{x + 1} doesn't overflow, it's always more than $n{x}. And it can't overflow, since that would be undefined behavior. So the compiler can replace the whole function with $c{return 0;}, and real compilers do exactly that. That's why a check for overflow that causes the overflow doesn't work. The right way is to compare $n{x} with $c{INT_MAX} before adding.",
    },
    {
      code: "int first(int *p) {\n  int x = *p;\n  if (p == NULL) {\n    return 0;\n  }\n  return x;\n}",
      choices: [
        "Remove the if statement, so it never returns 0 early.",
        "It has to keep the if statement, so that passing NULL returns 0.",
        "Move the check above line 2.",
      ],
      answer: 0,
      explanation:
        "If $n{p} were $c{NULL}, line 2 would already be undefined behavior. So after line 2, the compiler can assume $n{p} isn't $c{NULL}, and the check can never be true. A compiler removing a check exactly like this once caused a security hole in Linux.",
    },
    {
      code: "int a[8];\n\nvoid clear(void) {\n  for (int i = 0; i < 10; i++) {\n    a[i] = 0;\n  }\n}",
      choices: [
        "It has to write two zeros past the end of a.",
        "It has to stop after a[7].",
        "Anything at all: it doesn't have to write zeros past the end, stop after a[7], or even stop.",
      ],
      answer: 2,
      explanation:
        "Writing $c{a[8]} is undefined behavior, so the compiler can assume the loop never gets that far, and from that, that $c{i < 10} is always true. Real compilers have turned loops like this into infinite loops. There's no right answer for what this program does, because C doesn't give it one.",
    },
    {
      code: "int main(void) {\n  int x;\n  int y = x;\n  int z = x;\n  return y - z;\n}",
      choices: [
        "y and z are always equal, so main returns 0.",
        "y and z might be different.",
        "x is 0, since it was never given a value.",
      ],
      answer: 1,
      explanation:
        "Reading $n{x} is undefined behavior, since it has no value. It might not even have been given a place in memory or a register, and compilers really do sometimes give $n{y} and $n{z} different garbage.",
    },
    {
      code: "unsigned char lights;\n\nvoid blink(void) {\n  lights = 1;\n  lights = 0;\n}",
      choices: [
        "It has to turn the lights on, then off.",
        "It can skip lights = 1, since it's overwritten before anything reads it.",
        "It doesn't compile.",
      ],
      answer: 1,
      explanation:
        "There's no undefined behavior here, but the abstract machine can't tell whether the first store happened, so the optimizer can skip it. For real hardware, that would be a bug, since writing to the lights is the whole point. That's what $c{volatile} is for. $c{volatile unsigned char lights;} tells the compiler that every read and write matters, because something outside the program can see it, so none can be removed or combined. Memory-mapped hardware is always accessed through $c{volatile}, like $c{*(volatile unsigned char *)255 = 1;}.",
    },
    {
      code: "",
      question: false,
      explanation:
        "So undefined behavior isn't a particular result, like a crash or a garbage value. It's the absence of any rule at all, and optimizers take advantage of that. A program with undefined behavior can seem to work for years, and then break when it's compiled with a newer compiler or different settings. That's why C programmers work so hard to avoid it, and why there are tools to catch it, like the $c{-fsanitize=undefined} option of the gcc and clang compilers, which makes a program stop with an error when it does something undefined, much like the interpreter in this tutorial.",
    },
  ],
  next: "138-scope-and-storage.html",
});
