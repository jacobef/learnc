import { createProgramTemplate } from "./shared-program-template.js";

const basic = (ctx: { basicHint: string | null }) => ctx.basicHint;

createProgramTemplate({
  implicitMain: false,
  initialInstructions:
    "All of the statements so far that jump to another part of the program, like loops, $c{break}, and $c{continue}, are really just convenient, structured ways of doing the most basic jump there is: $c{goto}.\nA label is a name followed by a colon, written before a statement. It doesn't do anything by itself; it just marks a place. $c{goto label;} jumps straight to that place.\nClick $runLineButton to continue.",
  steps: [
    { code: "int main(void) {\n" },
    { code: "  int n = 1;\n" },
    { code: "again:\n" },
    { code: "  n *= 2;\n", editable: [false, true, false], hints: basic },
    {
      code: "  if (n < 5) {\n",
      editable: [false, true, false],
    },
    {
      code: "    goto again;\n",
      instructions: [
        "This jumps back to the $c{again} label. Execution continues from the statement after it, so this is a loop. Labels are usually written at the start of the line, so they stand out.",
        "",
      ],
    },
    { code: "  }\n" },
    {
      code: "  goto skip;\n",
      instructions: "A $c{goto} can jump forward, too, skipping over the code in between.",
    },
    { code: "  n = 100;\n" },
    { code: "skip:\n" },
    {
      code: "  n += 1;\n",
      editable: true,
      hints: (ctx) =>
        ctx.basicHintTopicIs("value", "n")
          ? "The $c{goto skip;} jumped over $c{n = 100;}, so this adds $v{1} to $n{n}'s value from the loop."
          : ctx.basicHint,
    },
    {
      code: "  return 0;\n",
      instructions:
        "A program full of $c{goto}s is very hard to follow, since any line might be jumped to from anywhere, so it's rarely used. Loops, $c{if}, $c{break}, and $c{continue} say what kind of jump is happening. But $c{goto} is how a computer really works underneath, and it's what the next statement, $c{switch}, is built on.",
    },
    { code: "}\n" },
  ],
  next: "120-switch.html",
});
