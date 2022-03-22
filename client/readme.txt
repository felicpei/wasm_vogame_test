wasm-pack build
wasm-pack test --headless --firefox
npm install
npm run start
npm run restart

wasm-pack build --target web
#run server


cargo run --bin veloren-server-cli


cargo build -Z unstable-options --profile debuginfo
cargo run -Z unstable-options --profile debuginfo

wasm-pack build --target web

npm install babel-preset-es2015