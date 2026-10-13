import { createProgramTemplate } from "./shared-program-template.js";
const basic = (ctx) => ctx.basicHint;
createProgramTemplate({
    implicitMain: false,
    initialInstructions: "Copying a big struct every time it's passed to a function is slow, and a function can't change a copy's original. So structs are often passed by pointer instead.\nClick $runLineButton to continue.",
    steps: [
        { code: "#include <stdlib.h>\n\n" },
        { code: "struct account {\n  int id;\n  int balance;\n};\n\n" },
        { code: "void deposit(struct account *a, int amount) {\n" },
        {
            code: "  a->balance += amount;\n",
            instructions: [
                "$c{a->balance} means $c{(*a).balance}: the $n{balance} member of the struct $n{a} points to. Since $c{.} has higher precedence than $c{*}, $c{*a.balance} would mean $c{*(a.balance)}, which is why the $c{->} operator exists.",
                "",
            ],
            editable: [false, true],
            hints: basic,
        },
        { code: "}\n\n" },
        { code: "int main(void) {\n" },
        { code: "  struct account mine = {1, 50};\n" },
        { code: "  deposit(&mine, 25);\n" },
        {
            code: "  struct account *other = malloc(sizeof(struct account));\n",
            instructions: "Structs are often allocated with $c{malloc}, especially when a program doesn't know in advance how many it'll need.",
        },
        { code: "  other->id = 2;\n" },
        { code: "  other->balance = mine.balance * 2;\n", editable: true, hints: basic },
        { code: "  deposit(other, -100);\n" },
        { code: "  return 0;\n" },
        { code: "}\n" },
    ],
    next: "143-midpoint.html",
});
