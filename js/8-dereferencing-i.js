import { createExpressionEvalTemplate } from "./shared-expression-template.js";
const abcSetup = `
int a;
int b;
int *c = &a;
`;
const defSetup = `
int d = 5;
int *e = &d;
int **f = &e;
`;
const ghijklSetup = `
int ***g;
int *h;
int **i;
int j = 0;
int *k;
int **l;
h = &j;
k = &j;
i = &k;
l = &h;
g = &l;
`;
createExpressionEvalTemplate({
    steps: [
        {
            expression: "*c",
            setup: abcSetup,
            instructions: "Here, the expression $c{*c} refers to $n{a}. For example, if the line $c{*c = 1;} were run, it would be equivalent to $c{a = 1;}.\nClick $nextButton to continue.",
        },
        {
            expression: "*f",
            setup: defSetup,
            instructions: "Here, $c{*f} refers to $n{e}. In general, $c{*X} refers to the variable whose address is $n{X}'s value.\nNote that this use of $c{*} is unrelated to the use of $c{*} in a type, like $t{int*}.\nClick $nextButton to continue.",
        },
        {
            expression: "*e",
            setup: defSetup,
            instructions: "Here, $c{*e} refers to $n{d}. In general, $c{*X} refers to the variable whose address is $n{X}'s value.\nClick $nextButton to continue.",
        },
        {
            expression: "*k",
            editable: true,
            fixValueCategory: true,
            setup: ghijklSetup,
            instructions: "Select the variable that the expression refers to, then press $checkButton.",
            hints: () => {
                return "$c{*k} refers to the variable whose address equals $n{k}'s value.";
            },
        },
        {
            expression: "*l",
            editable: true,
            fixValueCategory: true,
            setup: ghijklSetup,
            instructions: "Select the variable that the expression refers to, then press $checkButton.",
            hints: () => {
                return "$c{*l} refers to the variable whose address equals $n{l}'s value.";
            },
        },
        {
            expression: "**l",
            editable: true,
            fixValueCategory: true,
            setup: ghijklSetup,
            instructions: "Select the variable that the expression refers to, then press $checkButton.",
            hints: (ctx) => {
                if (ctx.selectedBox && ctx.selectedBox.name === "l") {
                    return "The $c{*}s don't cancel out, if that's what you were thinking. $c{**l} means $c{*(*l)}; that is, first determine what $c{*l} refers to, then apply $c{*} to that variable. For example, if $c{*l} refered to a variable named $n{x}, then $c{**l} would be equivalent to $c{*x}.";
                }
                if (ctx.selectedBox && ctx.selectedBox.name === "h") {
                    return "$n{h} is $c{*l}, not $c{**l}. You need to go 1 level deeper; that is, select the variable whose address is $n{h}'s value.";
                }
                return "$c{**l} means $c{*(*l)}; that is, first determine what $c{*l} refers to, then apply $c{*} to that variable. For example, if $c{*l} refered to a variable named $n{x}, then $c{**l} would be equivalent to $c{*x}.";
            },
        },
        {
            expression: "***g",
            editable: true,
            fixValueCategory: true,
            setup: ghijklSetup,
            instructions: "Select the variable that the expression refers to, then press $checkButton.",
            hints: (ctx) => {
                if (ctx.selectedBox && ctx.selectedBox.name === "l") {
                    return "$n{l} is $c{*g}, not $c{***g}. You need to go 2 levels deeper; that is, $c{***g} is equivalent to $c{**l}.";
                }
                if (ctx.selectedBox && ctx.selectedBox.name === "h") {
                    return "$n{h} is $c{**g}, not $c{***g}. You need to go 1 level deeper; that is, select the variable whose address is $n{h}'s value.";
                }
                return "$c{***g} means $c{*(*(*g))}. Follow the references one at a time.";
            },
        },
    ],
    workspace: { alwaysShowExprResult: false },
    next: "9-dereferencing-ii.html",
});
