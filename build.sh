dir=$(realpath $(dirname "$0"))

cd "$dir/client" \
    && wasm-pack build --target web \
    && cd "$dir" \
    && cargo build -p server \
    && content="$dir/target/debug/content" \
    && mkdir -p "$content" \
    && ln -sf "$dir/client/pkg" "$content/pkg" \
    && cp "$dir/client/index.html" "$content/index.html"
echo "serving $content"
RUST_BACKTRACE=1 "$dir/target/debug/server" "$content"
