.PHONY: all server client repl
all: server client

server: ./target/debug/conventional-commit-language-server
./target/debug/conventional-commit-language-server: ./src/*.rs ./Cargo.toml ./Cargo.lock
	cargo build

client: ./editors/code/src/tmLanguage.json ./editors/code/dist/main.min.common.js
./editors/code/src/tmLanguage.json: ./editors/code/src/tmLanguage.yaml
	./node_modules/.bin/js-yaml ./src/tmLanguage.yaml >./src/tmLanguage.json


editors/code/out/main.min.common.js: \
	editors/code/pnpm-lock.yaml \
	editors/code/package.json \
	editors/code/src/*.ts

	cd editors/code && esbuild ./src/main.ts --bundle --outfile=./dist/main.min.common.js \
		--format=cjs --platform=node --target=node18 \
		--external:vscode \
		--minify-{whitespace,identifiers,syntax} --sourcemap

repl: all
	code --extensionDevelopmentPath=./editors/code \
		--disable-extensions \
		./examples
