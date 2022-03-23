
import init, { start, set_resource_data } from "./pkg/veloren_voxygen.js";

(async function main() {
  await init();
  window.rust_func = {
    SetResourceData: set_resource_data,
  }
  DownAllRes(start)
})();

// import * as Comlink from 'comlink';
// (async function init() {
//     // Create a separate thread from wasm-worker.js and get a proxy to its handlers.
//     let handlers = await Comlink.wrap(
//       new Worker(new URL('./wasm_worker.js', import.meta.url), {
//         type: 'module'
//       })
//     ).handlers;

//     let handler;
//     if (await handlers.supportsThreads) {
//       handler = handlers["multiThread"]
//     }else{
//       handler = handlers["singleThread"]
//     }

//     window.rust_func = {
//       SetResourceData: handler.set_resource_data,
//     }
//     DownAllRes(handler.start);
// })();

