import * as wasm from "veloren_voxygen";

window.rust_func = {
    SetResourceData: wasm.set_resource_data,
}

(async function init() {
    // Create a separate thread from wasm-worker.js and get a proxy to its handlers.
    let handlers = await wasm.wrap(
      new Worker(new URL('./js/wasm-worker.js', import.meta.url), {
        type: 'module'
      })
    ).handlers;
  
    function setupBtn(id) {
        // Handlers are named in the same way as buttons.
        let handler = handlers[id];
        // If handler doesn't exist, it's not supported.
        if (!handler) return;
        // Assign onclick handler + enable the button.
        Object.assign(document.getElementById(id), {
            async onclick() {
                let { rawImageData, time } = await handler({
                width,
                height,
                maxIterations
                });
                timeOutput.value = `${time.toFixed(2)} ms`;
                const imgData = new ImageData(rawImageData, width, height);
                ctx.putImageData(imgData, 0, 0);
            },
            disabled: false
        });
    }

    setupBtn('singleThread');
    if (await handlers.supportsThreads) {
        setupBtn('multiThread');
    }

    //下载后，开始游戏
    DownAllRes(wasm.start);
})();
