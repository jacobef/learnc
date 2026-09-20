import { discoverSandboxBoundaries } from "./shared-sandbox-discovery.js";
import type { CSourceFile } from "./shared-c-interpreter.js";

const worker = self as unknown as {
  onmessage: (event: MessageEvent<{files: CSourceFile[]; stdin: string; implicitMain: boolean; stepLimit: number; addressBase: number}>) => void;
  postMessage: (message: ReturnType<typeof discoverSandboxBoundaries>) => void;
};
worker.onmessage = ({data}) => {
  let traceLimit = 1024;
  const discover = () => {
    const result = discoverSandboxBoundaries(data.files, data.stdin, data.implicitMain, data.stepLimit, traceLimit, data.addressBase);
    worker.postMessage(result);
    if (result.more) {
      traceLimit *= 2;
      setTimeout(discover, 0);
    }
  };
  discover();
};
