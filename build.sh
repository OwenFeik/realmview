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
if \
    [ ! -f "$schema" ] || \
    [ ! -f "$data/database.db" ] || \
    ! cmp --silent "$dir/server/schema.sql" "$schema"
then
    rm -f "$data/database.db"
    cp "$dir/server/schema.sql" "$schema"
    sqlite3 "$data/database.db" < "$schema"
fi

# Copy across content
content="$data/content"
mkdir -p "$content"
cp "$dir/client/web/"* "$content/"
if [ ! -d "$content/pkg" ]; then
    ln -sf "$dir/client/pkg" "$content/pkg"
fi

# Start server
echo "serving $content at http://localhost:3030/static/"
RUST_BACKTRACE=1 \
    DATABASE_URL="$data/database.db" \
    "$dir/target/debug/server" "$content"
