.PHONY: check fmt lint test install

BIN_DIR ?= $(HOME)/.local/bin

check: fmt lint test

fmt:
	cargo fmt --all --check
	sh -n release/install.sh

lint:
	cargo clippy --workspace --all-targets --locked -- -D warnings

test:
	cargo test --workspace --locked --quiet

# Builds from this checkout and installs the binary.
install:
	cargo build --locked --release -p tuse
	mkdir -p "$(BIN_DIR)"
	install -m 755 target/release/tuse "$(BIN_DIR)/tuse"
	@echo "Installed $$("$(BIN_DIR)/tuse" version) to $(BIN_DIR)/tuse"
	@case ":$$PATH:" in *":$(BIN_DIR):"*) ;; *) echo "Add $(BIN_DIR) to your PATH." ;; esac
