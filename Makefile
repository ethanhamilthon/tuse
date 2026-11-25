.PHONY: check fmt lint test install site

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

DOCS = usage connect config lua session json examples

# Builds the docs site from README.md and docs/ into target/book.
site:
	rm -rf target/book-src
	mkdir -p target/book-src
	cp README.md target/book-src/README.md
	mkdir -p target/book-src/docs && cp docs/screenshot.png docs/demo.gif target/book-src/docs/
	printf '# Summary\n\n[Tuse](README.md)\n\n' > target/book-src/SUMMARY.md
	for d in $(DOCS); do \
		cp docs/$$d.md target/book-src/$$d.md; \
		printf -- '- [%s](%s.md)\n' "$$(sed -n 's/^# //p;q' docs/$$d.md)" "$$d" >> target/book-src/SUMMARY.md; \
	done
	mdbook build
