{
  const { createProgramTemplate } = window.MB;

  function expectedValues(boundary) {
    if (boundary === 1) return { north: "" };
    if (boundary === 3) return { north: "5", south: "-5" };
    if (boundary === 5)
      return { north: "5", south: "-5", east: "9", west: "-9" };
    return {};
  }

  createProgramTemplate({
    lines: [
      "int north;",
      "int south = -5;",
      "north = 5;",
      "int east = 9;",
      "int west = -9;",
    ],
    editableSteps: [1, 3, 5],
    stepperFallback: false,
    next: "program4.html",
    workspace: { allowAddAndDelete: true },
    instructions: (ctx) => {
      if (ctx.boundary === 0) {
        return "No instructions for this one. Good luck!";
      }
      return null;
    },
    hints: {
      1: (ctx) => {
        const boxes = ctx.boxes || [];
        const byName = ctx.byName || {};
        if (boxes.length === 1) {
          if (!byName.north) {
            return "Name the new variable $n{north}.";
          } else if (byName.north.type !== "int") {
            return "$n{north}'s type should be $t{int}.";
          } else if (byName.north.value !== "") {
            return "$n{north}'s value should still be empty right after line 1.";
          } else return null;
        } else if (boxes.length === 0) {
          return "You need to add the $n{north} variable.";
        } else if (boxes.length === 2) {
          return "Line 1 should only add 1 new variable. Remove the extra variable.";
        } else {
          // boxes.length > 2
          return "Line 1 should only add 1 new variable. Remove the extra variables.";
        }
      },

      3: (ctx) => {
        const boxes = ctx.boxes || [];
        const byName = ctx.byName || {};
        if (boxes.length > 2) {
          return "Line 3 shouldn't create any new variables.";
        } else if (byName.north.value !== "5") {
          return "$n{north}'s value should be $v{5}.";
        } else if (byName.south.value !== "-5") {
          return "$n{south}'s value should remain $v{-5}.";
        } else return null;
      },

      5: (ctx) => {
        const boxes = ctx.boxes || [];
        const expected = { north: "5", south: "-5", east: "9", west: "-9" };
        const nExpected = 4;
        if (boxes.length === nExpected) {
          const byName = ctx.byName || {};
          if (!byName.west) {
            return "Name the new variable $n{west}.";
          } else if (byName.west.type !== "int") {
            return "$n{west}'s type should be $t{int}.";
          }
          for (const [name, expectedVal] of Object.entries(expected)) {
            if (byName[name] && byName[name].value !== expectedVal) {
              return `$n{${name}}'s value should ${name === "west" ? "be" : "remain"} $v{${expectedVal}}.`;
            }
          }
          return null;
        } else if (boxes.length < nExpected) {
          return "You need to add the $n{west} variable.";
        } else if (boxes.length === nExpected + 1) {
          return "Line 5 should only add 1 new variable. Remove the extra variable.";
        } else {
          // boxes.length > nExpected + 1
          return "Line 5 should only add 1 new variable. Remove the extra variables.";
        }
      },
    },
  });
}
