/// <reference path="./shared-program-template.ts" />

{
  const createProgramTemplate = window.MB!.createProgramTemplate!;

  createProgramTemplate({
    initialInstructions: "No instructions for this one. Good luck!",
    steps: [
      {
        code: "int north;\n",
        editable: true,
        hints: (ctx) => {
          const { boxes, getBoxByName } = ctx;
          const north = getBoxByName(boxes, "north");
          if (boxes.length === 1) {
            if (!north) {
              return "Name the new variable $n{north}.";
            } else if (north.type !== "int") {
              return "$n{north}'s type should be $t{int}.";
            } else if (north.value !== "") {
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
      },
      { code: "int east = 9;\n", editable: false },
      {
        code: "north = 5;\n",
        editable: true,
        hints: (ctx) => {
          const { boxes, getBoxesByName } = ctx;
          const [north, east] = getBoxesByName(boxes, "north", "east");
          if (boxes.length > 2) {
            return "Line 3 shouldn't create any new variables.";
          } else if (north!.value !== "5") {
            return "$n{north}'s value should be $v{5}.";
          } else if (east!.value !== "9") {
            return "$n{east}'s value should remain $v{9}.";
          } else return null;
        },
      },
      { code: "int south = -5;\n", editable: false },
      {
        code: "int west = -9;\n",
        editable: true,
        hints: (ctx) => {
          const { boxes, getBoxesByName } = ctx;
          if (boxes.length === 4) {
            const [north, east, south, west] = getBoxesByName(
              boxes,
              "north",
              "east",
              "south",
              "west",
            );
            if (!west) {
              return "Name the new variable $n{west}.";
            } else if (west.type !== "int") {
              return "$n{west}'s type should be $t{int}.";
            } else if (north && north.value !== "5") {
              return "$n{north}'s value should remain $v{5}.";
            } else if (east && east.value !== "9") {
              return "$n{east}'s value should remain $v{9}.";
            } else if (south && south.value !== "-5") {
              return "$n{south}'s value should remain $v{-5}.";
            } else if (west.value !== "-9") {
              return "$n{west}'s value should be $v{-9}.";
            }
            return null;
          } else if (boxes.length < 4) {
            return "You need to add the $n{west} variable.";
          } else if (boxes.length === 5) {
            return "Line 5 should only add 1 new variable. Remove the extra variable.";
          } else {
            // boxes.length > 5
            return "Line 5 should only add 1 new variable. Remove the extra variables.";
          }
        },
      },
    ],
    next: "program4.html",
    workspace: { allowVariableCreation: true },
  });
}
