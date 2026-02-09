import { createExpressionEvalTemplate } from "./shared-expression-template.js";
const defBoxes = [
    { name: "d", type: "int", value: "5", address: "292" },
    { name: "e", type: "int*", value: "292", address: "296" },
    { name: "f", type: "int**", value: "296", address: "308" },
];
const ghijklBoxes = [
    { name: "g", type: "int***", value: "292", address: "254" },
    { name: "h", type: "int*", value: "278", address: "262" },
    { name: "i", type: "int**", value: "286", address: "270" },
    { name: "j", type: "int", value: "0", address: "278" },
    { name: "k", type: "int*", value: "278", address: "286" },
    { name: "l", type: "int**", value: "262", address: "292" },
];
createExpressionEvalTemplate({
    steps: [
        {
            expression: "*c",
            editable: false,
            boxes: [
                { name: "a", type: "int", value: "", address: "108" },
                { name: "b", type: "int", value: "", address: "112" },
                { name: "c", type: "int*", value: "108", address: "116" },
            ],
            instructions: "In general, $c{*X} refers to the variable whose address is $n{X}'s value. Here, $c{*c} refers to $n{a}, so for example, $c{*c = 11;} would be equivalent to $c{a = 11;}.\nNote that this use of $c{*} is unrelated to the use of $c{*} in a type, like $t{int*}.",
        },
        {
            expression: "*f",
            editable: false,
            boxes: defBoxes,
            instructions: "In general, $c{*X} refers to the variable whose address is $n{X}'s value. Here, $c{*f} refers to $n{e}.",
        },
        {
            expression: "*e",
            editable: false,
            boxes: defBoxes,
            instructions: "In general, $c{*X} refers to the variable whose address is $n{X}'s value. Here, $c{*e} refers to $n{d}.",
        },
        {
            expression: "*l",
            editable: true,
            fixValueCategory: true,
            boxes: ghijklBoxes,
            instructions: "Select the variable that the expression refers to, then press $b{Check}.",
            hints: () => {
                return "$c{*l} refers to the variable whose address equals $n{l}'s value.";
            },
        },
        {
            expression: "*k",
            editable: true,
            fixValueCategory: true,
            boxes: ghijklBoxes,
            instructions: "Select the variable that the expression refers to, then press $b{Check}.",
            hints: () => {
                return "$c{*k} refers to the variable whose address equals $n{k}'s value.";
            },
        },
        {
            expression: "**l",
            editable: true,
            fixValueCategory: true,
            boxes: ghijklBoxes,
            instructions: "Select the variable that the expression refers to, then press $b{Check}.",
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
            boxes: ghijklBoxes,
            instructions: "Select the variable that the expression refers to, then press $b{Check}.",
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
    next: "9-dereferencing-ii.html",
});
