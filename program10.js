{
  const { createProgramTemplate } = window.MB;

  createProgramTemplate({
    lines: [
      "int a = 5 + 2 * (3 - 1);",
      "a = 1-3 * 4;",
      "a = (1 - 3) * 4;",
      "int b = 9 / 3;",
      "a = 5 / 3;",
      "a = -7 / 2;",
      "a = 1/2 + 1/2;",
      "a = 8 / -(2 + 1);",
      "int c = b+1 == 4;",
      "int d = b == 58;",
      "int e = 11/3 == 3;",
      "int f = 9 / 2+1 == 3;",
      "int g = 0 == 1 == 2;",
      "int h = (-2 / 3==1-1==1) - 3;",
    ],
    editableSteps: [2, 3, 8, 11, 12, 14],
    stepperFallback: true,
    next: "program11.html",
    workspace: { allowAddAndDelete: true },
    instructions: (ctx) => {
      if (ctx.boundary >= 1 && ctx.boundary <= 3) {
        return "The order of operations you know from math class (PEMDAS) applies, and isn't affected by spacing.";
      }
      if (ctx.boundary === 7) {
        return "Rounding happens at the division, so both 1/2s round to 0, so this becomes $c{a = 0 + 0;}, which is 0.";
      }
      if (ctx.boundary >= 5 && ctx.boundary <= 8) {
        return "Integer division rounds towards 0, i.e. it drops everything after the decimal point.";
      }
      if (ctx.boundary >= 9 && ctx.boundary <= 12) {
        return '$c{x == y} evaluates to 1 if x and y have equal values, and 0 if they don\'t. $c{==} has lower precedence than addition and subtraction.\n\nThe name of this operation is "equality".';
      }
      if (ctx.boundary === 13) {
        return "$c{==} is left-associative, so this is parsed as $c{(0 == 1) == 2}.";
      }
      if (ctx.boundary === 14) {
        return "Remember that spacing doesn't affect order of operations.";
      }
      return null;
    },
    hints: {
      2: (ctx) => {
        const by = ctx.byName || {};
        if (by.a.value === "-8") {
          return "You're parsing it as $c{(1 - 3) * 4}, but it should be parsed as $c{1 - (3 * 4)}, since multiplication has higher precedence than subtraction.";
        }
        if (by.a.value !== "-11") {
          return "Multiply before subtracting, then update $n{a}.";
        }
        return null;
      },
      3: (ctx) => {
        const by = ctx.byName || {};
        if (by.a.value !== "-8") {
          return "Evaluate the parentheses first, then multiply, then update $n{a}.";
        }
        return null;
      },
      8: (ctx) => {
        const by = ctx.byName || {};
        if (by.a.value !== "-2") {
          return "Compute the parentheses first, then negate, then divide, then update $n{a}.";
        }
        return null;
      },
      11: (ctx) => {
        const by = ctx.byName || {};
        if (!by.e) {
          return "Add the $n{e} variable.";
        }
        if (by.e.type !== "int") {
          return "$n{e}'s type should be $t{int}.";
        }
        if (by.e.value === "11") {
          return "You're parsing it as $c{11 / (3==3)}, but it should be parsed as $c{(11/3) == 3}. Division ($c{/}) has higher precedence than equality ($c{==}).";
        }
        if (by.e.value !== "1") {
          return "Calculate $c{11 / 3}, then drop everything after the decimal point. Is that equal to 3?";
        }
        return null;
      },
      12: (ctx) => {
        const by = ctx.byName || {};
        if (!by.f) {
          return "Add the $n{f} variable.";
        }
        if (by.f.type !== "int") {
          return "$n{f}'s type should be $t{int}.";
        }
        if (by.f.value === "1") {
          return "You're probably parsing it as $c{(9 / (2+1)) == 3}, but it should be parsed as $c{((9/2) + 1) == 3}. Division has higher precedence than addition.";
        }
        if (by.f.value === "4") {
          return "You're parsing it as $c{(9/2) + (1==3)}, but it should be parsed as $c{((9/2) + 1) == 3}. Division ($c{/}) has higher precedence than addition ($c{+}), which has higher precedence than equality ($c{==}).";
        }
        if (by.f.value !== "0") {
          return "Evaluate division ($c{/}), then addition ($c{+}), then equality ($c{==}).";
        }
        return null;
      },
      14: (ctx) => {
        const by = ctx.byName || {};
        if (!by.h) {
          return "Add the $n{h} variable.";
        }
        if (by.h.type !== "int") {
          return "$n{h}'s type should be $t{int}.";
        }
        if (by.h.value !== "-2") {
          return "The precedence order of the operations used in this line is, from highest to lowest: Parenthesis, division ($c{/}), subtraction ($c{-}), equality ($c{==}). Also recall that equality is left-associative, so $c{x == y == z} is parsed as $c{(x == y) == z}.";
        }
        return null;
      },
    },
  });
}
