import { createProgramTemplate } from "./shared-program-template.js";
createProgramTemplate({
    initialInstructions: "No instructions for this one, but the $referenceButton might come in handy. Good luck!",
    steps: [
        { code: "char word[] = \"code\";\n" },
        {
            code: "char *p = word + 1;\n",
            editable: true,
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "p")) {
                    return "$c{word} decays into a pointer to $c{word[0]}, so $c{word + 1} points to $c{word[1]}.";
                }
                return ctx.basicHint;
            },
        },
        {
            code: "int counts[3] = {0};\n",
            editable: true,
            hints: (ctx) => {
                const counts = ctx.boxNamed("counts");
                if (counts && !counts.arrayShape?.length) {
                    return "$n{counts} is an array of 3 $t{int}s, so its type is $t{int[3]}.";
                }
                return ctx.basicHint;
            },
        },
        {
            code: "*p = p[1] - 'a' + 'A';\n",
            editable: true,
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "word[1]")) {
                    return "$c{p[1]} is $c{word[2]}, the letter d. Subtracting $c{'a'} and adding $c{'A'} makes it uppercase. Then $n{*p}, which is $c{word[1]}, is set to that.";
                }
                return ctx.basicHint;
            },
        },
        {
            code: "for (int i = 0; word[i] != '\\0'; i++) {\n  if (word[i] >= 'a' && word[i] <= 'z') {\n    counts[0]++;\n  } else {\n    counts[1]++;\n  }\n}\n",
            editable: true,
            instructions: "What should the program state look like after this loop finishes?",
            hints: (ctx) => {
                if (ctx.boxNamed("i")) {
                    return "$n{i} was created in the loop's setup, so it should be removed when the loop finishes.";
                }
                if (ctx.basicHintTopicIs("value")) {
                    return "The loop visits each character of the string in $n{word}, counting lowercase letters in $c{counts[0]} and everything else in $c{counts[1]}. Remember that $c{word[1]} was changed on line 4.";
                }
                return ctx.basicHint;
            },
        },
        { code: "int *q = &counts[2];\n" },
        {
            code: "q[-1] += *(counts + 0);\n",
            editable: true,
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value")) {
                    return "$c{q[-1]} is the element before $c{counts[2]}, and $c{*(counts + 0)} is $c{counts[0]}.";
                }
                return ctx.basicHint;
            },
        },
        { code: "char *t = \"ab\";\n" },
        {
            code: "counts[2] = t[1] - t[0] + (word == p - 1);\n",
            editable: true,
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "counts[2]")) {
                    return "$c{t[1] - t[0]} is the difference between the codes for b and a. In $c{word == p - 1}, $c{word} decays into a pointer to $c{word[0]}; is that the same as $c{p - 1}?";
                }
                return ctx.basicHint;
            },
        },
    ],
    next: "107-hello-world.html",
    workspace: { allowVariableCreation: true, allowVariableDeletion: true },
});
