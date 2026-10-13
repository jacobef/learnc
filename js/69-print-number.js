import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";
createCodeOutputChallengeTemplate({
    inputs: [{ name: "n", type: "int" }],
    checkStdout: true,
    testInputs: ["0", "7", "10", "42", "100", "305", "9999", "123456", "2147483647"].map((n) => [n]),
    startInput: ["305"],
    prelude: "int putchar(int c);\n",
    main: ([n]) => `int main(void) {\n  print_number(${n});\n  putchar('\\n');\n  return 0;\n}`,
    solve: `void print_number(int n) {
  int place = 1;
  while (n / place >= 10) {
    place *= 10;
  }
  while (place > 0) {
    putchar('0' + n / place % 10);
    place /= 10;
  }
}`,
    textareaMinLines: 12,
    instructions: "Write a function named $c{print_number} that prints the digits of $n{n}, which won't be negative. For example, $c{print_number(305)} should print $c{305}. ($c{main} prints the newline.)\nThis one is tricky. Printing a single digit is easy: $c{putchar('0' + d)} prints the digit $n{d}. The hard part is getting the digits in the right order: $c{n % 10} gives the last digit, but it has to be printed last.\nThere are two good ways to do it: with a loop, or with recursion. With recursion, think about what has to be printed before the last digit.",
    hints: (ctx) => {
        if (ctx.behavesLike("void print_number(int n) {\n  while (n > 0) {\n    putchar('0' + n % 10);\n    n /= 10;\n  }\n}")) {
            return "That prints the digits in reverse order, and prints nothing for $v{0}. One fix: if $n{n} has more than one digit, $c{print_number(n / 10)} prints all of its digits except the last. Or, with a loop: try first finding the largest power of 10 that's at most $n{n} (for $v{305}, that's $v{100}). Then $c{n / 100 % 10} is the first digit, $c{n / 10 % 10} is the second, and $c{n / 1 % 10} is the last.";
        }
        if (ctx.currentResult.kind === "wrong-stdout") {
            return "With recursion: if $n{n} is at least $v{10}, $c{print_number(n / 10)} prints all of its digits except the last, and then only the last digit is left to print. With a loop: try first finding the largest power of 10 that's at most $n{n} (for $v{305}, that's $v{100}). Then $c{n / 100 % 10} is the first digit, $c{n / 10 % 10} is the second, and $c{n / 1 % 10} is the last. Be careful not to overflow an $t{int} while finding it.";
        }
        return null;
    },
    next: "70-include.html",
});
