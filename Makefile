.PHONY: check install

BIN_DIR ?= $(HOME)/.local/bin
CONFIG_DIR ?= $(HOME)/.config/tuse

check:
	cargo fmt --all --check
	cargo clippy --all-targets -- -D warnings
	cargo test --quiet

# Builds from this checkout and installs the binary.
# main.lua is created only when missing, so your own config stays.
install:
	cargo build --release -p tuse
	mkdir -p "$(BIN_DIR)" "$(CONFIG_DIR)"
	install -m 755 target/release/tuse "$(BIN_DIR)/tuse"
	@[ -e "$(CONFIG_DIR)/main.lua" ] || install -m 644 release/main.lua "$(CONFIG_DIR)/main.lua"
	@echo "Installed $$("$(BIN_DIR)/tuse" version) to $(BIN_DIR)/tuse, config in $(CONFIG_DIR)"
	@case ":$$PATH:" in *":$(BIN_DIR):"*) ;; *) echo "Add $(BIN_DIR) to your PATH." ;; esac
