import { discoverSandboxBoundaries } from "./shared-sandbox-discovery.js";
const worker = self;
worker.onmessage = ({ data }) => {
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
