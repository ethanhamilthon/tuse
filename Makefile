.PHONY: check build install

check:
	cargo fmt --all --check
	cargo clippy --all-targets -- -D warnings
	cargo test --quiet

build:
	cargo build --release

install:
	cargo install --path crates/cli
