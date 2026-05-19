default:
    @just --list

dev:
    @cargo run -p trek --features swagger

fmt:
    @cargo fmt
    @pnpm oxfmt .

[working-directory("./packages/ui")]
shadcn comment:
    @pnpm run shadcn add {{ comment }} --yes

[working-directory("./packages/api-types")]
api-types:
    @pnpm run generate-api-types

check:
    @cargo check --workspace

shear:
    @cargo shear --fix

install-wasm-tools:
	rustup target add wasm32-unknown-unknown
	cargo install wasm-bindgen-cli --version 0.2.121

compile-wasm:
	cargo build -p parser-wasm --target wasm32-unknown-unknown --release

bindgen-wasm target:
	wasm-bindgen target/wasm32-unknown-unknown/release/parser_wasm.wasm \
		--out-dir npm/trek-wasm-{{ target }} \
		--no-demangle \
		--target {{ target }} \
		--typescript 

	pnpm dlx wasm-opt npm/trek-wasm-{{ target }}/parser_wasm_bg.wasm \
		-o npm/trek-wasm-{{ target }}/parser_wasm_bg.wasm \
		-O3 \
		--enable-bulk-memory \
		--enable-nontrapping-float-to-int \
		--strip-debug

build-wasm:
	just compile-wasm
	just bindgen-wasm web

generate-schema:
	cargo run -p parser --example generate_schema > schema.json

next-test:
	cargo-nextest nextest run --all --all-targets