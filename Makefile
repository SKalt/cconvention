.PHONY: all
all: editors/code/out/main.min.common.js
	cargo build

editors/code/out/main.min.common.js: \
	editors/code/pnpm-lock.yaml \
	editors/code/package.json \
	editors/code/src/*.ts

	cd editors/code && pnpm build

