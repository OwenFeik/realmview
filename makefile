build := $(shell pwd)/build
content := ${build}/content

serve: server content
	echo "Serving at http://localhost:3030/"
	RUST_BACKTRACE=1 \
		DATABASE_URL=${build}/database.db \
		${build}/server ${content}

server: content database
	cargo build -p server
	cp --remove-destination target/debug/server ${build}/server

content: html
	wasm-pack build client/ --out-dir ${content}/pkg --target web

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

html: content-dir
	python3 client/web/build.py ${content}/

content-dir: build-dir
	mkdir -p ${content}

build-dir:
	mkdir -p ${build}

lint:
	cargo fmt
	cargo clippy
	python3 -m black client/web/build.py
	python3 -m mypy client/web/build.py

install:
	cargo install wasm-pack
	python3 -m pip install -r client/web/requirements.txt
