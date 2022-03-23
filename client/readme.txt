wasm-pack build --target web
#run server
cargo run --bin veloren-server-cli

wasm-pack build --target web

rollup ./index.js --format iife --file ../pkg/veloren_voxygen.js


//rollup need
set-ExecutionPolicy RemoteSigned


wasm-pack build --out-dir www/pkg --target web