import * as wasm from "veloren_voxygen";

window.rust_func = {
    SetResourceData: wasm.set_resource_data,
}

DownAllRes(wasm.start);
//wasm.start();