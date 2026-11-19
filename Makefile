.PHONY: check install

BIN_DIR ?= $(HOME)/.local/bin
CONFIG_DIR ?= $(HOME)/.config/tuse

check:
	@[ "$$(tr -d '[:space:]' < release/VERSION)" = "$$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)" ] || { echo "VERSION and Cargo.toml differ"; exit 1; }
	cargo fmt --all --check
	cargo clippy --all-targets -- -D warnings
	cargo test --quiet

# Builds from this checkout and installs the binary and default.yaml (replaced, like `tuse update`).
# main.yaml is created only when missing, so your own config stays.
install:
	cargo build --release -p tuse
	mkdir -p "$(BIN_DIR)" "$(CONFIG_DIR)"
	install -m 755 target/release/tuse "$(BIN_DIR)/tuse"
	install -m 644 release/default.yaml "$(CONFIG_DIR)/default.yaml"
	@[ -e "$(CONFIG_DIR)/main.yaml" ] || install -m 644 release/main.yaml "$(CONFIG_DIR)/main.yaml"
	@echo "Installed $$("$(BIN_DIR)/tuse" version) to $(BIN_DIR)/tuse, config in $(CONFIG_DIR)"
	@case ":$$PATH:" in *":$(BIN_DIR):"*) ;; *) echo "Add $(BIN_DIR) to your PATH." ;; esac
