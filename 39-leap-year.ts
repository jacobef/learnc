import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";

const testInputs: string[][] = [
  "1", "4", "100", "400", "1600", "1700", "1800", "1900", "1996", "1999",
  "2000", "2001", "2004", "2020", "2023", "2024", "2100", "2400", "2500",
].map((year) => [year]);

createCodeOutputChallengeTemplate({
  inputs: [{ name: "year", type: "int" }],
  outputs: [{ name: "leap", type: "int" }],
  startInput: ["2024"],
  testInputs,
  solve: "int leap = year % 4 == 0 && year % 100 != 0 || year % 400 == 0;",
  textareaMinLines: 8,
  instructions:
    "Write code that creates an $t{int} variable named $n{leap}, whose value is $v{1} if $n{year} is a leap year, and $v{0} otherwise.\nA year is a leap year if it's divisible by 4, except for years divisible by 100, which are only leap years if they're also divisible by 400. For example, 2024 and 2000 are leap years, but 2023 and 1900 are not.",
  hints: (ctx) => {
    if (ctx.behavesLike("int leap = year % 4 == 0;")) {
      return "Years divisible by 100 are usually not leap years. For example, 1900 was not a leap year.";
    }
    if (ctx.behavesLike("int leap = year % 4 == 0 && year % 100 != 0;")) {
      return "Close, but years divisible by 400 are always leap years. For example, 2000 was a leap year.";
    }
    if (ctx.currentResult.kind === "wrong-output-value") {
      const leap = ctx.currentResult.outputBox?.value;
      if (leap !== "0" && leap !== "1") {
        return "$n{leap} should be either $v{0} or $v{1}.";
      }
      return "Recall from $i{36. Remainder} that $c{x % y == 0} checks whether $c{x} is divisible by $c{y}.";
    }
    return null;
  },
  next: "40-undefined-behavior.html",
});
