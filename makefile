# All targets are just scripts; there are no file dependencies. Thus commands
# should never be skipped.
MAKEFLAGS += --always-make

root := $(shell pwd)
build := ${root}/build
target := ${build}/target
content := ${build}/content
env := CARGO_TARGET_DIR=${target}
cargo := ${env} cargo
wp := RUST_BACKTRACE=1 ${env} wasm-pack
py := python3
dep := ${HOME}/deployment

serve: server content testdb
	echo "Serving at http://localhost:3030/"
	RUST_BACKTRACE=1                      \
		DATA_DIR=${build}                 \
		DATABASE_URL=${build}/database.db \
		${build}/server 3030

deploy: html deploydb
	${cargo} build -p server --release
	${wp} build --release client/ --out-dir ${content}/pkg --target web
	cp --remove-destination ${target}/release/server ${dep}/server
	cp -r ${content} ${dep}/content
	sudo setcap CAP_NET_BIND_SERVICE=+eip ${dep}/server
	echo "Serving on port 80"
	RUST_BACKTRACE=1 DATABASE_URL=${dep}/database.db DATA_DIR=${dep} \
		${dep}/server 80

deploydb: database
	mkdir -p ${dep}
	@if [ -f ${dep}/database.db ]; then                                     \
		mkdir -p ${dep}/backups;                                            \
		echo "Backing up database.";                                        \
		cp ${dep}/database.db ${dep}/backups/$$(date "+database_%F_%T.db"); \
	fi
	@if                                                    \
		[ ! -f ${dep}/schema.sql ]                         \
		|| [ ! -f ${dep}/database.db ]                     \
		|| ! cmp -s ${build}/schema.sql ${dep}/schema.sql; \
	then                                                   \
		echo "Rebuilding database. Migration required.";   \
		cp ${build}/schema.sql ${dep}/schema.sql; 		   \
		cp ${build}/database.db ${dep}/database.db;        \
	fi

server: content database
	${cargo} build -p server
	cp --remove-destination ${target}/debug/server ${build}/server

content: html wasm

db: testdb
	sqlite3 ${build}/database.db --header --box || \
		sqlite3 ${build}/database.db --header

testdb: database
	sqlite3 ${build}/database.db < ${root}/server/sql/test_data.sql \
		2>/dev/null || true

database: build-dir
	@if \
		[ ! -f ${build}/schema.sql ] \
		|| [ ! -f ${build}/database.db ] \
		|| ! cmp -s ${root}/server/sql/schema.sql ${build}/schema.sql; \
	then \
		rm -rf ${content}/uploads; \
		rm -f ${build}/database.db; \
		cp ${root}/server/sql/schema.sql ${build}/schema.sql; \
		sqlite3 ${build}/database.db < ${build}/schema.sql; \
	fi

wasm: content-dir
	${wp} build client/ --out-dir ${content}/pkg --target web --dev

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

test: test-py test-rust

test-rust:
	export DATA_DIR=$$(mktemp -d)                                     \
	&& echo "Running tests with DATA_DIR=$$DATA_DIR"                  \
	&& sqlite3 $$DATA_DIR/database.db < ${root}/server/sql/schema.sql \
	&& DATABASE_URL=sqlite://$$DATA_DIR/database.db ${cargo} test

test-py:
	cd ${root}/web && ${py} test.py

install:
	${cargo} install wasm-pack
	${py} -m pip install -r ${root}/web/requirements.txt

clean:
	rm -rf ${build}
	rm -rf .mypy_cache
	rm -rf web/include/.cache
