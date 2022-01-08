wasm-pack build --target web

if [ $1 == "-s" ]; then
    python -m http.server
fi
