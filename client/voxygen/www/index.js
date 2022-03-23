
import init, { start, set_resource_data } from "../pkg/veloren_voxygen.js";

async function main() {
  await init();
  window.rust_func = {
    SetResourceData: set_resource_data,
  }
  DownAllRes(start)
}

main();