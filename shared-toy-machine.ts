// A tiny made-up computer, for showing what C turns into. It has 256 bytes of
// memory, four 8-bit registers (A, B, C, and D), a stack pointer, and eight lights
// wired to the bits of the byte at LIGHTS_ADDRESS.
//
// Its assembly language puts the destination first, like a C assignment:
//   SET A, 5         A = 5
//   COPY A, B        A = B
//   LOAD A, [200]    A = the byte at address 200 (or [B]: the address in B)
//   STORE [200], A   the byte at address 200 = A (or [B])
//   ADD A, B         A = A + B (also SUB, AND, OR, XOR; the source can be a number)
//   SHL A, 1         A = A << 1 (also SHR)
//   NOT A            A = ~A
//   JMP label        jump to label
//   JZ A, label      jump to label if A is zero (JNZ: if it isn't)
//   PUSH A / POP A   push onto, or pop off, the stack
//   CALL label / RET call a subroutine, and return from it
//   HALT             stop
// Arithmetic wraps around, as with unsigned char. Labels are "name:", and comments
// start with ";". Numbers can be decimal, 0b binary, or 0x hexadecimal.

export const LIGHTS_ADDRESS = 255;
export const STACK_START = 254;
export const REGISTERS = ["A", "B", "C", "D"] as const;
export type Register = (typeof REGISTERS)[number];

export interface MachineState {
  registers: Record<Register, number>;
  sp: number;
  memory: number[];
}

type Operand =
  | { kind: "register"; register: Register }
  | { kind: "number"; value: number }
  | { kind: "address"; address: number }
  | { kind: "pointer"; register: Register }
  | { kind: "label"; label: string };

export interface MachineStart {
  registers?: Partial<Record<Register, number>>;
  memory?: Record<number, number>;
}

export interface Instruction {
  line: number;
  op: string;
  operands: Operand[];
}

export interface MachineProgram {
  instructions: Instruction[];
  labels: Map<string, number>;
}

export interface MachineStep {
  // The instruction that ran, and the line it's on.
  index: number;
  line: number;
  op: string;
  // Where execution goes next, or null once the program has stopped.
  nextIndex: number | null;
  jumped: boolean;
  after: MachineState;
}

export interface MachineRun {
  steps: MachineStep[];
  final: MachineState;
  // Why the program stopped, if it wasn't by halting or running off the end.
  error: string | null;
}

export class MachineError extends Error {
  constructor(message: string, readonly line: number) {
    super(message);
  }
}

const SHAPES: Record<string, string[]> = {
  SET: ["register", "number"],
  COPY: ["register", "register"],
  LOAD: ["register", "memory"],
  STORE: ["memory", "register"],
  ADD: ["register", "value"],
  SUB: ["register", "value"],
  AND: ["register", "value"],
  OR: ["register", "value"],
  XOR: ["register", "value"],
  SHL: ["register", "value"],
  SHR: ["register", "value"],
  NOT: ["register"],
  JMP: ["label"],
  JZ: ["register", "label"],
  JNZ: ["register", "label"],
  PUSH: ["register"],
  POP: ["register"],
  CALL: ["label"],
  RET: [],
  HALT: [],
};

export const JUMPS = new Set(["JMP", "JZ", "JNZ", "CALL", "RET"]);

function parseNumber(text: string): number | null {
  const match = /^(-?)(0b[01]+|0x[0-9a-f]+|\d+)$/i.exec(text);
  if (!match) return null;
  const digits = match[2]!.toLowerCase();
  const value = digits.startsWith("0b")
    ? parseInt(digits.slice(2), 2)
    : digits.startsWith("0x")
      ? parseInt(digits.slice(2), 16)
      : parseInt(digits, 10);
  return match[1] ? -value : value;
}

function parseOperand(text: string, expected: string, line: number): Operand {
  const upper = text.toUpperCase();
  const register = (REGISTERS as readonly string[]).includes(upper) ? (upper as Register) : null;
  const what = {
    register: "a register (A, B, C, or D)",
    number: "a number",
    value: "a register or a number",
    memory: "a memory address in brackets, like [200] or [B]",
    label: "a label",
  }[expected];
  const fail = (): never => {
    throw new MachineError(`Expected ${what}, but found "${text}".`, line);
  };
  if (expected === "register") return register ? { kind: "register", register } : fail();
  if (expected === "number" || expected === "value") {
    if (register && expected === "value") return { kind: "register", register };
    const value = parseNumber(text);
    if (value === null) return fail();
    return { kind: "number", value };
  }
  if (expected === "memory") {
    const inner = /^\[\s*(.*?)\s*\]$/.exec(text)?.[1];
    if (inner === undefined) return fail();
    const pointer = (REGISTERS as readonly string[]).includes(inner.toUpperCase());
    if (pointer) return { kind: "pointer", register: inner.toUpperCase() as Register };
    const address = parseNumber(inner);
    if (address === null || address < 0 || address > 255) {
      throw new MachineError(`Memory addresses go from 0 to 255, but found "${inner}".`, line);
    }
    return { kind: "address", address };
  }
  return /^[A-Za-z_]\w*$/.test(text) ? { kind: "label", label: text } : fail();
}

export function assemble(source: string): MachineProgram {
  const instructions: Instruction[] = [];
  const labels = new Map<string, number>();
  source.split("\n").forEach((rawLine, line) => {
    let text = rawLine.replace(/;.*$/, "").trim();
    const label = /^([A-Za-z_]\w*)\s*:/.exec(text);
    if (label) {
      if (labels.has(label[1]!)) throw new MachineError(`The label ${label[1]} is defined twice.`, line);
      labels.set(label[1]!, instructions.length);
      text = text.slice(label[0].length).trim();
    }
    if (!text) return;
    const [, opText, rest = ""] = /^(\S+)\s*(.*)$/.exec(text)!;
    const op = opText!.toUpperCase();
    const shape = SHAPES[op];
    if (!shape) throw new MachineError(`There's no instruction named ${opText}.`, line);
    const parts = rest.trim() ? rest.split(",").map((part) => part.trim()) : [];
    if (parts.length !== shape.length) {
      throw new MachineError(
        `${op} takes ${shape.length === 0 ? "nothing" : shape.length === 1 ? "1 operand" : `${shape.length} operands`}, but was given ${parts.length}.`,
        line,
      );
    }
    instructions.push({ line, op, operands: parts.map((part, i) => parseOperand(part, shape[i]!, line)) });
  });
  for (const instruction of instructions) {
    for (const operand of instruction.operands) {
      if (operand.kind === "label" && !labels.has(operand.label)) {
        throw new MachineError(`There's no label named ${operand.label}.`, instruction.line);
      }
    }
  }
  return { instructions, labels };
}

export function emptyMachine(): MachineState {
  return { registers: { A: 0, B: 0, C: 0, D: 0 }, sp: STACK_START, memory: new Array(256).fill(0) };
}

export function copyMachine(state: MachineState): MachineState {
  return { registers: { ...state.registers }, sp: state.sp, memory: state.memory.slice() };
}

const byte = (value: number) => ((value % 256) + 256) % 256;

export function runMachine(program: MachineProgram, start: MachineState, stepLimit = 5000): MachineRun {
  const state = copyMachine(start);
  const steps: MachineStep[] = [];
  let pc = 0;
  const value = (operand: Operand) =>
    operand.kind === "register" ? state.registers[operand.register] : operand.kind === "number" ? byte(operand.value) : 0;
  const address = (operand: Operand) =>
    operand.kind === "pointer" ? state.registers[operand.register] : operand.kind === "address" ? operand.address : 0;
  const reg = (operand: Operand) => (operand as { register: Register }).register;
  const target = (operand: Operand) => program.labels.get((operand as { label: string }).label)!;
  while (pc < program.instructions.length) {
    if (steps.length >= stepLimit) {
      return { steps, final: state, error: `The program ran ${stepLimit} instructions without halting. Is there a loop that never ends?` };
    }
    const instruction = program.instructions[pc]!;
    const [first, second] = instruction.operands;
    let next: number | null = pc + 1;
    switch (instruction.op) {
      case "SET":
      case "COPY":
        state.registers[reg(first!)] = value(second!);
        break;
      case "LOAD":
        state.registers[reg(first!)] = state.memory[address(second!)]!;
        break;
      case "STORE":
        state.memory[address(first!)] = state.registers[reg(second!)];
        break;
      case "ADD":
      case "SUB":
      case "AND":
      case "OR":
      case "XOR":
      case "SHL":
      case "SHR": {
        const a = state.registers[reg(first!)];
        const b = value(second!);
        const result = {
          ADD: a + b,
          SUB: a - b,
          AND: a & b,
          OR: a | b,
          XOR: a ^ b,
          SHL: b >= 8 ? 0 : a << b,
          SHR: b >= 8 ? 0 : a >> b,
        }[instruction.op]!;
        state.registers[reg(first!)] = byte(result);
        break;
      }
      case "NOT":
        state.registers[reg(first!)] = byte(~state.registers[reg(first!)]);
        break;
      case "JMP":
        next = target(first!);
        break;
      case "JZ":
        if (state.registers[reg(first!)] === 0) next = target(second!);
        break;
      case "JNZ":
        if (state.registers[reg(first!)] !== 0) next = target(second!);
        break;
      case "PUSH":
        state.memory[state.sp] = state.registers[reg(first!)];
        state.sp = byte(state.sp - 1);
        break;
      case "POP":
        state.sp = byte(state.sp + 1);
        state.registers[reg(first!)] = state.memory[state.sp]!;
        break;
      case "CALL":
        // The return address is the index of the instruction after the call.
        state.memory[state.sp] = pc + 1;
        state.sp = byte(state.sp - 1);
        next = target(first!);
        break;
      case "RET":
        state.sp = byte(state.sp + 1);
        next = state.memory[state.sp]!;
        break;
      case "HALT":
        next = null;
        break;
    }
    if (next !== null && next >= program.instructions.length) next = null;
    steps.push({
      index: pc,
      line: instruction.line,
      op: instruction.op,
      nextIndex: next,
      jumped: next !== null && next !== pc + 1,
      after: copyMachine(state),
    });
    if (next === null) break;
    pc = next;
  }
  return { steps, final: state, error: null };
}

export function startState(start: MachineStart | undefined): MachineState {
  const state = emptyMachine();
  for (const [register, value] of Object.entries(start?.registers ?? {})) {
    state.registers[register as Register] = value!;
  }
  for (const [address, value] of Object.entries(start?.memory ?? {})) {
    state.memory[Number(address)] = value;
  }
  return state;
}
