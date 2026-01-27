import { createProgramTemplate } from "./shared-program-template.js";
createProgramTemplate({
    initialInstructions: "Click $runLineButton to continue.",
    steps: [
        {
            code: "double clown = 4.5;\n",
            editable: false,
        },
        {
            code: "int juggler = clown;\n",
            editable: false,
            instructions: "When an $t{int} is assigned to a $t{double}, the decimal part is dropped.",
        },
        {
            code: "double circus = juggler;\n",
            editable: false,
            instructions: "When a $t{double} is assigned to an $t{int}, the mathematical value is preserved. In this case, $v{4} is simply converted to $v{4.0}.",
        },
        {
            code: "circus = 5.0/2.0;\n",
            editable: true,
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "circus")) {
                    const circus = ctx.boxNamed("circus");
                    if (circus.value === "2" || circus.value === "2.0") {
                        return "Both operands are $t{double}s, so keep the decimal part: $v{2.5}.";
                    }
                    return "Compute $c{5.0/2.0} using $t{double} division.";
                }
                return ctx.basicHint;
            },
        },
        {
            code: "circus = 5/2;\n",
            editable: true,
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "circus")) {
                    const circus = ctx.boxNamed("circus");
                    if (circus.value === "2.5") {
                        return "$c{5/2} uses $t{int} division.";
                    }
                    if (circus.value === "2") {
                        return "$c{5/2} evalutes to $v{2}, then assigning it to a $t{double} makes it $v{2.0}. Enter $v{2.0}, not $v{2}.";
                    }
                    return "Compute $c{5/2} using $t{int} division, then convert the result to a $t{double} (since $n{circus} is a $t{double}).";
                }
                return ctx.basicHint;
            },
        },
        {
            code: "juggler = 5/2;\n",
            editable: true,
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "juggler")) {
                    const juggler = ctx.boxNamed("juggler");
                    if (juggler.value === "3" || juggler.value === "3.0") {
                        return "$t{int} division doesn't round, it drops the decimal part.";
                    }
                    if (juggler.value === "2.5") {
                        return "$c{5/2} is $t{int} division.";
                    }
                    if (juggler.value === "2.0") {
                        return "$n{juggler} is an $t{int}, so enter $v{2}, not $v{2.0}.";
                    }
                    return "Evaluate $c{5/2} with $t{int} division, then update $n{juggler}.";
                }
                return ctx.basicHint;
            },
        },
        {
            code: "juggler = 5.0/2.0;\n",
            editable: true,
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "juggler")) {
                    const juggler = ctx.boxNamed("juggler");
                    if (juggler.value === "2.5" || juggler.value === "2.0") {
                        return "$c{5.0/2.0} is $v{2.5}, but assigning to an $t{int} should then drop the decimal part.";
                    }
                    if (juggler.value === "3" || juggler.value === "3.0") {
                        return "Assigning to an $t{int} drops the decimal part.";
                    }
                    return "Compute $c{5.0/2.0} with $t{double} division, then convert the result to an $t{int} (since $n{juggler} is an $t{int}).";
                }
                return ctx.basicHint;
            },
        },
        {
            code: "double* ring = &circus;\n",
            editable: false,
        },
        {
            code: "double** tent = &ring;\n",
            editable: true,
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "tent")) {
                    const [tent, ring, circus] = ctx.boxesNamed("tent", "ring", "circus");
                    if (tent.value === ring.value) {
                        return "$n{tent} should store $n{ring}'s address, not $n{ring}'s value.";
                    }
                    return "Set $n{tent}'s value to $n{ring}'s address.";
                }
                return ctx.basicHint;
            },
        },
        {
            code: "*tent = &clown;\n",
            editable: true,
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "ring")) {
                    const [ring, clown, circus] = ctx.boxesNamed("ring", "clown", "circus");
                    if (ring.value === clown.value) {
                        return "$n{ring} should store $n{clown}'s address, not $n{clown}'s value.";
                    }
                    return "$n{*tent} refers to the variable whose address is $n{tent}'s value. Set $n{*tent}'s value to $n{clown}'s address.";
                }
                return ctx.basicHint;
            },
        },
        {
            code: "*ring = 10/3;\n",
            editable: true,
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "clown")) {
                    const clown = ctx.boxNamed("clown");
                    if (clown.value === "3") {
                        return "$n{clown} is a $t{double}, so enter $v{3.0}, not $v{3}.";
                    }
                    if (clown.value.startsWith("3.") && clown.value !== "3.0") {
                        return "$c{10/3} uses $t{int} division, so the decimal part is dropped before storing it as a $t{double}.";
                    }
                    return "$n{*ring} refers to the variable whose address is $n{ring}'s value.";
                }
                return ctx.basicHint;
            },
        },
    ],
    next: "16-blocks.html",
    workspace: { allowVariableCreation: true },
});
