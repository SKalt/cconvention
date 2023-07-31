# FIXME: this build process is pretty convoluted. It should be simplified.
.PHONY: all help server client-js client bin tmLanguage vsix repl never
all: bin vsix

help:
	@echo "targets:"
	@echo "  all (default): build server and client"
	@echo "  server: build server"
	@echo "  client-js: build vscode client"
	@echo "  client: assemble vscode client"
	@echo "  vsix: package vscode extension"
	@echo "environment variables:"
	@echo "  PROFILE: debug (default), release"
	@echo "  VERSION: base (default), pro"
PROFILE?=debug
# allowed values: debug, release
VERSION?=base
# allowed values: pro, base

never: # used to invalidate non-phony targets (real files) that should always run

server: ./target/${PROFILE}/${VERSION}_language_server
# TODO: generate cargo build timing, bloat metrics
./target/debug/${VERSION}_language_server: never
	cargo build --all-features --bin ${VERSION}_language_server --timings

./target/release/${VERSION}_language_server: never
	cargo build --all-features --release --bin ${VERSION}_language_server --timings

bin: ./bin/conventional-commit-language-server
./bin/conventional-commit-language-server: ./target/${PROFILE}/${VERSION}_language_server
	cd ./bin && \
	rm -f ${VERSION}_language_server && \
	rm -f conventional-commit-language-server && \
	ln -s ../target/${PROFILE}/${VERSION}_language_server ${VERSION}_language_server && \
	ln -s ../target/${PROFILE}/${VERSION}_language_server conventional-commit-language-server

tmLanguage: ./editors/code/base/src/tmLanguage.json
./editors/code/base/src/tmLanguage.json: ./editors/code/base/src/tmLanguage.yaml
	cd ./editors/code/base && PATH=$(shell pwd)/node_modules/.bin:${PATH} ./scripts/build_textmate.sh
# retained as a symlink in the pro version

client-js: ./editors/code/${VERSION}/dist/main.min.common.js
./editors/code/base/dist/main.min.common.js: \
	editors/code/base/pnpm-lock.yaml \
	editors/code/base/src/*.ts

	cd editors/code/base && ./scripts/build_js.sh

./editors/code/pro/dist/main.min.common.js: ./editors/code/base/dist/main.min.common.js
	cp ./editors/code/base/dist/main.min.common.js ./editors/code/pro/dist/main.min.common.js

client: client-js tmLanguage ./editors/code/${VERSION}/dist/conventional-commit-language-server
./editors/code/${VERSION}/dist/conventional-commit-language-server: \
	./target/${PROFILE}/${VERSION}_language_server

	cp ./target/${PROFILE}/${VERSION}_language_server ./editors/code/${VERSION}/dist/conventional-commit-language-server

vsix: \
	./editors/code/${VERSION}/dist/conventional-commit-language-server.vsix \
	./editors/code/${VERSION}/dist/main.min.common.js \
	./editors/code/${VERSION}/src/tmLanguage.json \
	./target/${PROFILE}/${VERSION}_language_server

./editors/code/${VERSION}/dist/conventional-commit-language-server.vsix:
	cd editors/code/${VERSION} && ./scripts/build_vsix.sh

clean-bin:
	rm -f ./bin/${VERSION}_language_server
	rm -f ./bin/conventional-commit-language-server

clean-vsix:
	rm -f ./editors/code/${VERSION}/dist/conventional-commit-language-server.vsix
