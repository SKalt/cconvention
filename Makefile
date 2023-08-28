# FIXME: this build process is pretty convoluted. It should be simplified.
.PHONY: all help server client-js client bin tmLanguage vsix repl never test lint
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
# ^allowed values: debug, release

VERSION?=base
# ^allowed values: pro, base

TARGET?=x86_64-unknown-linux-gnu
# ^allowed values: whatever your `rustup target list` says

never: # used to invalidate non-phony targets (real files) that should always run

server: ./target/${PROFILE}/${VERSION}_language_server
# TODO: generate cargo build timing, bloat metrics
./target/${PROFILE}/${VERSION}_language_server: never
	./scripts/build_bin.sh \
		--profile=${PROFILE} \
		--version=${VERSION} \
		--target=${TARGET}
	touch -m ./target/${PROFILE}/${VERSION}_language_server

bin: ./bin/cconvention
./bin/cconvention: ./target/${PROFILE}/${VERSION}_language_server
	mkdir -p ./bin && \
	cd ./bin && \
	rm -f ${VERSION}_language_server && \
	rm -f cconvention && \
	ln -s ../target/${PROFILE}/${VERSION}_language_server ${VERSION}_language_server && \
	ln -s ../target/${PROFILE}/${VERSION}_language_server cconvention

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

client: client-js tmLanguage ./editors/code/${VERSION}/dist/cconvention
./editors/code/${VERSION}/dist/cconvention: \
	./target/${PROFILE}/${VERSION}_language_server

	mkdir -p ./editors/code/${VERSION}/dist && \
		cp ./target/${PROFILE}/${VERSION}_language_server ./editors/code/${VERSION}/dist/cconvention

vsix: ./editors/code/${VERSION}/dist/cconvention.vsix

./editors/code/${VERSION}/dist/cconvention.vsix: \
	./editors/code/${VERSION}/dist/cconvention \
	./editors/code/${VERSION}/dist/main.min.common.js \
	./editors/code/${VERSION}/src/tmLanguage.json \
	./editors/code/${VERSION}/package.json \
	./scripts/build_vsix.sh

	./scripts/build_vsix.sh \
		--profile=${PROFILE} \
		--version=${VERSION} \
		--target=${TARGET}

clean-bin:
	rm -f ./bin/${VERSION}_language_server
	rm -f ./bin/cconvention

clean-vsix:
	rm -f ./editors/code/${VERSION}/dist/cconvention.vsix

test:
	cargo test --all-features
	./scripts/run_checks.sh

lint:
	cargo clippy --all-features
	./scripts/link_check.sh
	./scripts/shellcheck.sh
