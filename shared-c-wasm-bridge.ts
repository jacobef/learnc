import { C_INTERPRETER_WASM_BASE64 } from "./shared-c-interpreter-wasm-data.js";
import type {
  CExecutionBudget,
  CSourceFile,
} from "./shared-c-interpreter-types.js";

interface CInterpreterExports extends WebAssembly.Exports {
  memory: WebAssembly.Memory;
  cboxes_alloc: (length: number) => number;
  cboxes_free: (pointer: number, length: number) => void;
  cboxes_last_result_len: () => number;
  cboxes_execute: (requestPointer: number, requestLength: number) => number;
}

type CBridgeOperation =
  | "run-source"
  | "run-files"
  | "evaluate-source"
  | "evaluate-files";

interface CBridgeRequest {
  operation: CBridgeOperation;
  primary: string | Uint8Array;
  expression?: string;
  stdin: string;
  syntheticAddressBase: number;
  eventIndex?: number;
  implicitMain?: boolean;
  executionBudget: CExecutionBudget;
}

// This identifies the one supported wire shape; it is not a compatibility version.
const BRIDGE_SCHEMA_ID = 1;
const BRIDGE_HEADER_WORDS = 10;
const BRIDGE_FLAG_IMPLICIT_MAIN = 1;
const OPERATION_CODES: Record<CBridgeOperation, number> = {
  "run-source": 0,
  "run-files": 1,
  "evaluate-source": 2,
  "evaluate-files": 3,
};

let exportsCache: CInterpreterExports | null = null;
const textEncoder = new TextEncoder();
const textDecoder = new TextDecoder();

class WasiProcExit extends Error {
  readonly code: number;

  constructor(code: number) {
    super(`interpreter exited through WASI proc_exit(${code})`);
    this.name = "WasiProcExit";
    this.code = code;
  }
}

function decodeBase64(data: string): Uint8Array {
  const binary = atob(data);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}

function writeU32(memory: WebAssembly.Memory, pointer: number, value: number) {
  new DataView(memory.buffer).setUint32(pointer, value, true);
}

function writeU64(memory: WebAssembly.Memory, pointer: number, value: bigint) {
  new DataView(memory.buffer).setBigUint64(pointer, value, true);
}

function wasiImports(getExports: () => CInterpreterExports | null) {
  const errnoBadf = 8;
  const errnoNosys = 52;
  const errnoNotsup = 58;
  const ok = 0;
  return {
    clock_time_get: (clockId: number, _precision: bigint, timePtr: number) => {
      const memory = getExports()?.memory;
      if (!memory) return errnoNosys;
      const nanoseconds =
        clockId === 0
          ? BigInt(Date.now()) * 1_000_000n
          : clockId === 1
            ? BigInt(Math.floor(performance.now() * 1_000_000))
            : null;
      if (nanoseconds === null) return errnoNotsup;
      writeU64(memory, timePtr, nanoseconds);
      return ok;
    },
    environ_get: () => ok,
    environ_sizes_get: (countPtr: number, sizePtr: number) => {
      const memory = getExports()?.memory;
      if (memory) {
        writeU32(memory, countPtr, 0);
        writeU32(memory, sizePtr, 0);
      }
      return ok;
    },
    fd_close: () => ok,
    fd_prestat_get: () => errnoBadf,
    fd_prestat_dir_name: () => errnoBadf,
    fd_seek: () => errnoNosys,
    fd_write: (_fd: number, _iovs: number, _iovsLen: number, nwrittenPtr: number) => {
      const memory = getExports()?.memory;
      if (memory) writeU32(memory, nwrittenPtr, 0);
      return ok;
    },
    proc_exit: (code: number) => {
      throw new WasiProcExit(code);
    },
    random_get: (pointer: number, length: number) => {
      const memory = getExports()?.memory;
      if (!memory) return errnoNosys;
      const bytes = new Uint8Array(memory.buffer, pointer, length);
      for (let offset = 0; offset < bytes.length; offset += 65_536) {
        crypto.getRandomValues(bytes.subarray(offset, offset + 65_536));
      }
      return ok;
    },
  };
}

function interpreterExports(): CInterpreterExports {
  if (exportsCache) return exportsCache;
  let current: CInterpreterExports | null = null;
  const bytes = Uint8Array.from(decodeBase64(C_INTERPRETER_WASM_BASE64));
  const module = new WebAssembly.Module(bytes as unknown as BufferSource);
  const instance = new WebAssembly.Instance(module, {
    env: { clock: () => Math.floor(Date.now() / 1000) },
    wasi_snapshot_preview1: wasiImports(() => current),
  });
  current = instance.exports as CInterpreterExports;
  exportsCache = current;
  return current;
}

function encodeSourceFiles(files: CSourceFile[]): Uint8Array {
  const encoded = files.map((file) => ({
    path: textEncoder.encode(file.path),
    source: textEncoder.encode(file.source),
  }));
  const byteLength =
    4 +
    encoded.reduce(
      (total, file) => total + 8 + file.path.length + file.source.length,
      0,
    );
  const output = new Uint8Array(byteLength);
  const view = new DataView(output.buffer);
  let offset = 0;
  view.setUint32(offset, encoded.length, true);
  offset += 4;
  for (const file of encoded) {
    view.setUint32(offset, file.path.length, true);
    view.setUint32(offset + 4, file.source.length, true);
    offset += 8;
    output.set(file.path, offset);
    offset += file.path.length;
    output.set(file.source, offset);
    offset += file.source.length;
  }
  return output;
}

function encodeBridgeRequest(request: CBridgeRequest): Uint8Array {
  const primary =
    typeof request.primary === "string"
      ? textEncoder.encode(request.primary)
      : request.primary;
  const expression = textEncoder.encode(request.expression ?? "");
  const stdin = textEncoder.encode(request.stdin);
  const headerBytes = BRIDGE_HEADER_WORDS * 4;
  const output = new Uint8Array(
    headerBytes + primary.length + expression.length + stdin.length,
  );
  const view = new DataView(output.buffer);
  const word = (index: number, value: number) =>
    view.setUint32(index * 4, Math.max(0, Math.floor(value)), true);
  word(0, BRIDGE_SCHEMA_ID);
  word(1, OPERATION_CODES[request.operation]);
  word(2, request.implicitMain ? BRIDGE_FLAG_IMPLICIT_MAIN : 0);
  word(3, request.syntheticAddressBase);
  word(4, request.eventIndex ?? 0);
  word(5, request.executionBudget.stepLimit);
  word(6, request.executionBudget.followingTraceLimit);
  word(7, primary.length);
  word(8, expression.length);
  word(9, stdin.length);
  let offset = headerBytes;
  output.set(primary, offset);
  offset += primary.length;
  output.set(expression, offset);
  offset += expression.length;
  output.set(stdin, offset);
  return output;
}

function invokeCInterpreter<T>(request: CBridgeRequest): T {
  const interpreter = interpreterExports();
  const bytes = encodeBridgeRequest(request);
  const requestPointer = interpreter.cboxes_alloc(bytes.length);
  let outputPointer: number | null = null;
  let outputLength = 0;
  try {
    new Uint8Array(interpreter.memory.buffer).set(bytes, requestPointer);
    outputPointer = interpreter.cboxes_execute(requestPointer, bytes.length);
    outputLength = interpreter.cboxes_last_result_len();
    const json = textDecoder.decode(
      new Uint8Array(interpreter.memory.buffer, outputPointer, outputLength),
    );
    return JSON.parse(json) as T;
  } finally {
    try {
      interpreter.cboxes_free(requestPointer, bytes.length);
      if (outputPointer != null) {
        interpreter.cboxes_free(outputPointer, outputLength);
      }
    } catch {
      exportsCache = null;
    }
  }
}

function resetCInterpreterBridge(): void {
  exportsCache = null;
}

export {
  encodeSourceFiles,
  invokeCInterpreter,
  resetCInterpreterBridge,
  WasiProcExit,
};
export type { CBridgeOperation, CBridgeRequest };
