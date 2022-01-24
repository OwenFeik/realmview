set -e

dir=$(realpath $(dirname "$0"))

if [ "$1" == "hard" ]; then
    rm -rf "$dir/target"
fi

# Build client
cd "$dir/client"
wasm-pack build --target web

# Build server
cd "$dir"
cargo build -p server

data="$dir/target/debug/data"
mkdir -p "$data"

# Create database if schema changed / not run
schema="$data/schema.sql"
if [ ! -f "$schema" ] || cmp --silent "$dir/server/schema.sql" "$schema"; then
    rm -f "$data/database.db"
    cp "$dir/server/schema.sql" "$data/"
    sqlite3 "$data/database.db" < "$data/schema.sql"
fi

# Copy across content
content="$data/content"
mkdir -p "$content"
ln -sf "$dir/client/pkg" "$content/pkg"
cp "$dir/client/index.html" "$content/"

# Start server
echo "serving $content at http://localhost:3030/static/"
RUST_BACKTRACE=1 \
    DATABASE_URL="$data/database.db" \
    "$dir/target/debug/server" "$content"
