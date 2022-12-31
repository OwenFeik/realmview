root := $(shell pwd)
build := ${root}/build
target := ${build}/target
content := ${build}/content
cargo := CARGO_TARGET_DIR=${target} RUST_BACKTRACE=1 cargo
py := python3

serve: server content
	echo "Serving at http://localhost:3030/"
	RUST_BACKTRACE=1 \
		DATABASE_URL=${build}/database.db \
		${build}/server ${content} 3030

deploy: content
	${cargo} build -p server --release
	cp --remove-destination ${target}/release/server ${build}/server
	echo "Serving on port 80"
	sudo \
		RUST_BACKTRACE=1 \
		DATABASE_URL=${build}/database.db \
		${build}/server ${content} 80

server: content database
	${cargo} build -p server
	cp --remove-destination ${target}/debug/server ${build}/server

content: html wasm

db: database
	sqlite3 ${build}/database.db --header --box || \
		sqlite3 ${build}/database.db --header

database: build-dir
	@if \
		[ ! -f ${build}/schema.sql ] \
		|| [ ! -f ${build}/database.db ] \
		|| ! cmp -s ${root}/server/sql/schema.sql ${build}/schema.sql; \
	then \
		rm -rf ${content}/uploads \
		rm -f ${build}/database.db; \
		cp ${root}/server/sql/schema.sql ${build}/schema.sql; \
		sqlite3 ${build}/database.db < ${build}/schema.sql; \
		sqlite3 ${build}/database.db < ${root}/server/sql/test_data.sql; \
	fi

wasm: content-dir
	CARGO_TARGET_DIR=${target} \
		wasm-pack build client/ --out-dir ${content}/pkg --target web --dev

html: content-dir
	${py} ${root}/web/build.py ${content}/

content-dir: build-dir
	mkdir -p ${content}

build-dir:
	mkdir -p ${build}

approve: lint test

lint: lint-rust lint-py

lint-rust:
	${cargo} fmt
	${cargo} clippy

lint-py:
	${py} -m black ${root}/web/
	MYPY_CACHE_DIR=${build}/.mypy_cache ${py} -m mypy ${root}/web/

test: test-rust test-py

test-rust:
	${cargo} test

test-py:
	cd ${root}/web && ${py} test.py

install:
	${cargo} install wasm-pack
	${py} -m pip install -r ${root}/web/requirements.txt

clean:
	rm -rf ${build}
	rm -rf .mypy_cache
	rm -rf web/include/.cache
