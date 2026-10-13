import { createProgramTemplate } from "./shared-program-template.js";
const basic = (ctx) => ctx.basicHint;
createProgramTemplate({
    implicitMain: false,
    initialInstructions: "An array holds several values of the same type. A struct holds several values of possibly different types, each with its own name, called members. Defining a struct creates a new type.\nClick $runLineButton to continue.",
    steps: [
        { code: "struct point {\n  int x;\n  int y;\n};\n\n" },
        { code: "int main(void) {\n" },
        {
            code: "  struct point p;\n",
            instructions: "Lines 1 to 4 define a type named $t{struct point}, with two $t{int} members, $n{x} and $n{y}. They don't create any variables. This line creates a variable of that type.",
        },
        {
            code: "  p.x = 3;\n",
            instructions: "$c{p.x} is $n{p}'s member named $n{x}. Each member is a variable in its own right.",
        },
        { code: "  p.y = p.x * 2;\n", editable: true, hints: basic },
        {
            code: "  struct point q = {5, -1};\n",
            instructions: "A struct can be initialized with a list, like an array. The values go to the members in order.",
        },
        {
            code: "  struct point r = {.y = 9};\n",
            editable: true,
            instructions: "A list can also name the members, with designated initializers. Members left out are set to $v{0}, like with arrays.",
            hints: basic,
        },
        {
            code: "  int sum = q.x + r.x + r.y;\n",
            editable: true,
            hints: basic,
        },
        { code: "  return 0;\n" },
        { code: "}\n" },
    ],
    next: "126-structs-ii.html",
    workspace: { allowVariableCreation: true },
});
