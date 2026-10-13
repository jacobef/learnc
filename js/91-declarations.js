import { createProgramTemplate } from "./shared-program-template.js";
// Declaration and use trees are drawn side by side; matching nodes share a color,
// and each node of a use is labeled with its type.
const node = (text, pair, note, child) => ({
    text,
    kind: child ? "operator" : "name",
    pair,
    note,
    children: child ? [child] : undefined,
});
createProgramTemplate({
    initialInstructions: "Types like $t{int[3]} and $t{int*} are written differently in declarations: the declaration is $c{int a[3];}, not $c{int[3] a;}. That's because a declaration isn't written as \"type, then name\". Instead, it's written as a statement about how the name is used: $c{int a[3];} says \"$c{a[i]} is an $t{int}\". From that, it follows that $n{a} is an array of $t{int}s.\nThe part after the type, like $c{a[3]}, is called the declarator, and it's grouped just like the expression it describes. Below, the declarator and a use of $n{a} are each drawn as a tree, with matching parts in matching colors. The declaration says that the top of the use, $c{a[i]}, is an $t{int}. Working down from there gives the type of each part, down to $n{a} itself: if $c{a[i]} is an $t{int}, then $n{a} is an array of $t{int}s, and the declarator says there are 3 of them.\nClick $runLineButton to continue.",
    initialDiagrams: [
        { title: "Declaration: int a[3];", tree: node("[3]", 1, undefined, node("a", 2)) },
        { title: "Use: a[i]", tree: node("[i]", 1, "int", node("a", 2, "int[3]")) },
    ],
    steps: [
        { code: "int a[3] = {1, 2, 3};\n" },
        {
            code: "int *p = &a[0];\n",
            instructions: "Likewise, $c{int *p;} says \"$c{*p} is an $t{int}\", so $n{p} is a pointer to an $t{int}.\nSpaces don't matter here: $c{int *p} and $c{int* p} mean the same thing. This tutorial has been writing $c{int* p}, since it's easier to read as \"type, then name\" before you know how declarations really work. But from now on, it'll write $c{int *p}, since that matches the reading \"$c{*p} is an $t{int}\". Many C programmers prefer it for a reason you'll see at the end of this lesson.",
            diagrams: [
                { title: "Declaration: int *p;", tree: node("*", 1, undefined, node("p", 2)) },
                { title: "Use: *p", tree: node("*", 1, "int", node("p", 2, "int *")) },
            ],
        },
        {
            code: "int **pp = &p;\n",
            editable: true,
            instructions: "Working down the use tree: $c{**pp} is an $t{int}, so $c{*pp} is a pointer to an $t{int}, so $n{pp} is a pointer to a pointer to an $t{int}. To write a type on its own, take a declaration and remove the name: $c{int **pp} becomes $t{int **}.",
            diagrams: [
                { title: "Declaration: int **pp;", tree: node("*", 1, undefined, node("*", 2, undefined, node("pp", 3))) },
                { title: "Use: **pp", tree: node("*", 1, "int", node("*", 2, "int *", node("pp", 3, "int **"))) },
            ],
            hints: (ctx) => ctx.basicHintTopicIs("type", "pp")
                ? "$n{pp} is a pointer to a pointer to an $t{int}: remove the name from $c{int **pp} to get $t{int **}."
                : ctx.basicHint,
        },
        {
            code: "int x = 4, *y = &x, z[2];\n",
            editable: true,
            instructions: "One declaration can create several variables, separated by commas. Each one has its own declarator, combined with the same type: here, $c{x} is an $t{int}, $c{*y} is an $t{int}, and $c{z[i]} is an $t{int}.",
            diagrams: [
                { title: "Use: x", tree: node("x", 1, "int") },
                { title: "Use: *y", tree: node("*", 1, "int", node("y", 2, "int *")) },
                { title: "Use: z[i]", tree: node("[i]", 1, "int", node("z", 2, "int[2]")) },
            ],
            hints: (ctx) => ctx.basicHint,
        },
        {
            code: "int* m, n;\n",
            editable: true,
            instructions: "Careful: what are $n{m}'s and $n{n}'s types?",
            hints: (ctx) => {
                if (ctx.boxNamed("n")?.type.replace(/\s+/g, "") === "int*") {
                    return "The $c{*} belongs to $n{m}'s declarator, not to the type, no matter where the space is. This says $c{*m} is an $t{int} and $c{n} is an $t{int}. This trap is why many programmers write $c{int *m, n;}.";
                }
                return ctx.basicHint;
            },
        },
    ],
    next: "92-declarations-ii.html",
    workspace: { allowVariableCreation: true },
});
