wasm-pack build
wasm-pack test --headless --firefox
npm install
npm run start
npm run restart

wasm-pack build --target web
#run server
cargo run --bin veloren-server-cli


SET RUST_LOG=debug cargo run