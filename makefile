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
		${build}/server ${content}

server: content database
	${cargo} build -p server
	cp --remove-destination ${target}/debug/server ${build}/server

content: html wasm

db: database
	sqlite3 ${build}/database.db --header --box

database: build-dir
	@if \
		[ ! -f ${build}/schema.sql ] \
		|| [ ! -f ${build}/database.db ] \
		|| ! cmp -s ${root}/server/sql/schema.sql ${build}/schema.sql; \
	then \
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

lint: test
	python3 -m black ${root}/web/build.py
	MYPY_CACHE_DIR=${build}/.mypy_cache ${py} -m mypy ${root}/web/build.py
	${cargo} fmt
	${cargo} clippy

test:
	${cargo} test

install:
	${cargo} install wasm-pack
	${py} -m pip install -r ${root}/web/requirements.txt

clean:
	rm -rf ${build}
	rm -rf .mypy_cache
	rm -rf web/include/.cache
