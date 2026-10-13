import { createProgramTemplate } from "./shared-program-template.js";
const basic = (ctx) => ctx.basicHint;
createProgramTemplate({
    implicitMain: false,
    initialInstructions: "An enumeration, or enum, defines a list of named integer constants, along with a type for them. It's a way to give names to a small set of possibilities.\nClick $runLineButton to continue.",
    steps: [
        { code: "enum direction { NORTH, EAST, SOUTH, WEST };\n\n" },
        {
            code: "enum direction turn_right(enum direction d) {\n",
        },
        { code: "  return (d + 1) % 4;\n" },
        { code: "}\n\n" },
        { code: "int main(void) {\n" },
        {
            code: "  enum direction facing = NORTH;\n",
            instructions: "The constants are numbered in order, starting at $v{0}, so $c{NORTH} is $v{0}, $c{EAST} is $v{1}, and so on. An $c{enum} variable is really just an integer, which is why the program state shows a number.",
        },
        { code: "  facing = turn_right(facing);\n", editable: [false, true], hints: basic },
        {
            code: "  facing = turn_right(WEST);\n",
            editable: [false, true],
            hints: (ctx) => ctx.basicHintTopicIs("value", "facing") ? "$c{WEST} is $v{3}, and $c{(3 + 1) % 4} is $v{0}." : ctx.basicHint,
        },
        {
            code: "  enum level { LOW = 1, MEDIUM = 5, HIGH };\n",
            instructions: "A constant can be given a value. One that isn't is one more than the constant before it.",
        },
        { code: "  int high = HIGH;\n", editable: true, hints: basic },
        { code: "  return 0;\n" },
        { code: "}\n" },
    ],
    next: "148-unions.html",
});
