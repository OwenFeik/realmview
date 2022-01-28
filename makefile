build := $(shell pwd)/build
content := ${build}/content

serve: server content
	echo "Serving at http://localhost:3030/"
	RUST_BACKTRACE=1 \
		DATABASE_URL=${build}/database.db \
		${build}/server ${content}

server: content database
	cargo build -p server
	cp target/debug/server ${build}/server

content: build-dir
	mkdir -p ${content}
	wasm-pack build client/ --out-dir ${content}/pkg --target web
	cp client/web/* ${content}/

database: build-dir
	if \
		[ ! -f ${build}/schema.sql ] \
		|| [ ! -f ${build}/database.db ] \
		|| ! cmp -s server/schema.sql ${build}/schema.sql; \
	then \
		rm -f ${build}/database.db; \
		cp server/schema.sql ${build}/schema.sql; \
		sqlite3 ${build}/database.db < ${build}/schema.sql; \
	fi

build-dir:
	mkdir -p ${build}
