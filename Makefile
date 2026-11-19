.PHONY: check

check:
	@[ "$$(tr -d '[:space:]' < release/VERSION)" = "$$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)" ] || { echo "VERSION and Cargo.toml differ"; exit 1; }
	cargo fmt --all --check
	cargo clippy --all-targets -- -D warnings
	cargo test --quiet
