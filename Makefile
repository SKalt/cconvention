# FIXME: this build process is pretty convoluted. It should be simplified.
.PHONY: all help server client-js vsix repl
all: server vsix

help:
	@echo "targets:"
	@echo "  all (default): build server and client"
	@echo "environment variables:"
	@echo "  TARGET: debug (default), release"
	@echo "  VERSION: base (default), pro"
TARGET?=debug
# allowed values: debug, release
VERSION?=base
# allowed values: pro, base

server: ./target/${TARGET}/${VERSION}_language_server
./target/debug/${VERSION}_language_server:
	cargo build --all-features --bin ${VERSION}_language_server
./target/release/${VERSION}_language_server:
	cargo build --all-features --release --bin ${VERSION}_language_server

tmLanguage: ./editors/code/${VERSION}/src/tmLanguage.json
./editors/code/base/src/tmLanguage.json: ./editors/code/base/src/tmLanguage.yaml
	cd ./editors/code/base && PATH=$(shell pwd)/node_modules/.bin:${PATH} ./scripts/build_textmate.sh
./editors/code/pro/src/tmLanguage.json: ./editors/code/base/src/tmLanguage.json
	cp ./editors/code/base/src/tmLanguage.json ./editors/code/pro/src/tmLanguage.json


client-js: ./editors/code/${VERSION}/dist/main.min.common.js
./editors/code/base/dist/main.min.common.js: \
	editors/code/base/pnpm-lock.yaml \
	editors/code/base/src/*.ts

	cd editors/code/base && ./scripts/build_js.sh

./editors/code/pro/dist/main.min.common.js: ./editors/code/base/dist/main.min.common.js
	cp ./editors/code/base/dist/main.min.common.js ./editors/code/pro/dist/main.min.common.js

vsix: \
	./editors/code/${VERSION}/dist/conventional-commit-language-server.vsix \
	./editors/code/${VERSION}/dist/main.min.common.js \
	./editors/code/${VERSION}/src/tmLanguage.json \
	./target/${TARGET}/${VERSION}_language_server

./editors/code/${VERSION}/dist/conventional-commit-language-server.vsix:
	cd editors/code/${VERSION} && ./scripts/build_vsix.sh

clean-vsix:
	rm -f ./editors/code/${VERSION}/dist/conventional-commit-language-server.vsix
