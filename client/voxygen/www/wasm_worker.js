
import { threads } from 'wasm-feature-detect';
import * as Comlink from 'comlink';

async function initHandlers() {
  let [singleThread, multiThread] = await Promise.all([
    (async () => {
      const singleThread = await import('./pkg/veloren_voxygen.js');
      await singleThread.default();
      return singleThread;
    })(),
    (async () => {
      // If threads are unsupported in this browser, skip this handler.
      if (!(await threads())) {
        console.error("unsupported multiThread in this browser");
        return;
      } 
      const multiThread = await import('./pkg/veloren_voxygen.js');
      await multiThread.default();
      let threadCount = navigator.hardwareConcurrency;
      await multiThread.initThreadPool(threadCount);

      console.log("使用多线程，线程数:" + threadCount);
      return multiThread;
    })()
  ]);

  return Comlink.proxy({
    singleThread,
    supportsThreads: !!multiThread,
    multiThread
  });
}

Comlink.expose({
  handlers: initHandlers()
});
