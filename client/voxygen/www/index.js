import * as wasm from "wasm_test";

window.rust_func = {
    SetResourceData: wasm.set_resource_data,
}

DownAllRes(wasm.start);
//wasm.start();