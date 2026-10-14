import { createProgramTemplate } from "./shared-program-template.js";
createProgramTemplate({
    initialInstructions: "Arrays don't decay when they're the operand of $c{sizeof}. So $c{sizeof} gives the size of the whole array, which is something you can't find out from a pointer.\nClick $runLineButton to continue.",
    steps: [
        { code: "int a[5] = {3, 1, 4, 1, 5};\n" },
        {
            code: "int bytes = sizeof a;\n",
            editable: true,
            instructions: "How many bytes does $n{a} take up?",
            hints: (ctx) => ctx.basicHintTopicIs("value", "bytes") ? "$n{a} is 5 $t{int}s, and each $t{int} takes up 4 bytes." : ctx.basicHint,
        },
        {
            code: "int count = sizeof a / sizeof a[0];\n",
            editable: true,
            instructions: "Dividing the array's size by the size of one element gives the number of elements. This is a common idiom, since it keeps working if the array's length is changed later.",
            hints: (ctx) => ctx.basicHintTopicIs("value", "count") ? "$c{sizeof a} is $v{20}, and $c{sizeof a[0]} is the size of one $t{int}." : ctx.basicHint,
        },
        { code: "int* p = a;\n" },
        {
            code: "int pointer_bytes = sizeof p;\n",
            editable: true,
            instructions: "$n{p} points to $n{a}'s first element, but it's just a pointer: it doesn't know how many elements come after the one it points to. So a function that's given a pointer to an array's first element has to be told the array's length separately.",
            hints: (ctx) => ctx.basicHintTopicIs("value", "pointer_bytes") ? "$c{sizeof p} is the size of a pointer." : ctx.basicHint,
        },
    ],
    next: "87-pointer-loops.html",
    workspace: { allowVariableCreation: true, allowVariableDeletion: true },
});
