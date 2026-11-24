import init, { parseWim } from "../pkg/wim_parser_wasm.js";

// Initialize exactly once, including when several requests arrive together.
const ready = init();
// Attach an immediate handler to avoid an unhandled rejection before a request.
ready.catch(() => {});

self.onmessage = async ({ data }) => {
  const { id, file } = data;
  try {
    if (!(file instanceof File)) throw new TypeError("A File is required");
    await ready;
    const result = await parseWim(file);
    self.postMessage({ id, result });
  } catch (error) {
    self.postMessage({ id, error: error instanceof Error ? error.message : String(error) });
  }
};
