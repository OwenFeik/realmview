dir=$(realpath $(dirname "$0"))

cd "$dir/client" \
    && wasm-pack build --target web \
    && cd "$dir" \
    && cargo build -p server \
    && data="$dir/target/debug/data" \
    && mkdir -p "$data" \
    && content="$data/content" \
    && mkdir -p "$content" \
    && ln -sf "$dir/client/pkg" "$content/pkg" \
    && cp "$dir/client/index.html" "$content/" \
    && sqlite3 "$data/database.db" < "$dir/server/schema.sql" \
    && echo "serving $content" \
    && RUST_BACKTRACE=1 "$dir/target/debug/server" "$data"
