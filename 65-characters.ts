import { createProgramTemplate } from "./shared-program-template.js";

createProgramTemplate({
  initialInstructions: "Click $runLineButton to continue.",
  steps: [
    {
      code: "char c = 65;\n",
      instructions:
        "$t{char} is another integer type, like $t{int}, but much smaller: it takes up 1 byte, and typically holds values from $v{-128} to $v{127}.\nIts values are used as codes for characters, using a standard called ASCII. For example, $v{65} is the code for the letter A. The program state shows the character under each $t{char}'s value.",
    },
    {
      code: "char d = 'B';\n",
      instructions:
        "$c{'B'} is a character literal. It's just another way to write the number $v{66}, the code for B. The $t{char} still holds a number; nothing about it is special except how it's displayed.",
    },
    {
      code: "d = d + 1;\n",
      editable: true,
      instructions:
        "The letters A to Z have consecutive codes, from $v{65} to $v{90}. (The $referenceButton has a table of the most important ones.)",
      hints: (ctx) => ctx.basicHint,
    },
    {
      code: "char digit = '7';\n",
      editable: true,
      instructions:
        "The digit characters $c{'0'} to $c{'9'} have codes $v{48} to $v{57}. Note that $c{'7'} is the code for the character 7, which is not the same as the number $v{7}.",
      hints: (ctx) => {
        if (ctx.boxNamed("digit")?.value === "7") {
          return "$c{'7'} is the character 7, whose code is $v{55}, not the number $v{7}.";
        }
        return ctx.basicHint;
      },
    },
    {
      code: "int value = digit - '0';\n",
      editable: true,
      instructions:
        "Since the digit characters have consecutive codes, subtracting $c{'0'} turns a digit character into the number it stands for.",
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "value")) {
          return "$c{digit - '0'} is $c{55 - 48}.";
        }
        return ctx.basicHint;
      },
    },
    { code: "char lower = 'e';\n" },
    {
      code: "char upper = lower - 'a' + 'A';\n",
      editable: true,
      instructions:
        "Lowercase letters are consecutive too, from $c{'a'} ($v{97}) to $c{'z'} ($v{122}). So $c{lower - 'a'} is how far into the alphabet $n{lower} is.",
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "upper")) {
          return "$c{lower - 'a'} is $v{4}, since e is 4 letters after a. Then $c{4 + 'A'} is the letter 4 after A.";
        }
        return ctx.basicHint;
      },
    },
    {
      code: "char newline = '\\n';\n",
      instructions:
        "Some characters are written with a backslash. $c{'\\n'} is the newline character, which marks the end of a line of text; its code is $v{10}. We'll use it soon.",
    },
    {
      code: "char nothing = '\\0';\n",
      editable: true,
      instructions:
        "$c{'\\0'} is the null character. Its code is $v{0}. It doesn't stand for any visible character, but it has an important use that we'll see in the next few lessons.",
      hints: (ctx) => ctx.basicHint,
    },
  ],
  next: "66-uppercase.html",
  workspace: { allowVariableCreation: true },
});
