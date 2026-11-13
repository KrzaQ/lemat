# Lemat: a math board that works with a coding agent.
#
# Machine-specific values (the git remotes to push to) go in Makefile.local,
# which git ignores. See Makefile.local.example.

.PHONY: help deps build install icon run test format lint push hooks check-private clean
.DEFAULT_GOAL := help

REMOTES ?= origin
APP_DIR ?= $(HOME)/Applications
BIN_DIR ?= $(HOME)/.local/bin
BUNDLE := src-tauri/target/release/bundle/macos/Lemat.app

help: ## Show this help
	@grep -hE '^[a-zA-Z0-9_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort -t: -k1,1 | \
		awk 'BEGIN {FS = ":.*?## "; w = 12} { k[NR] = $$1; v[NR] = $$2; if (length($$1) > w) w = length($$1) } \
		     END { for (i = 1; i <= NR; i++) printf "  \033[36m%-*s\033[0m  %s\n", w, k[i], v[i] }'

deps: ## Install the Tauri CLI and other npm dependencies
	npm install

build: deps lejce/Cargo.toml ## Build Lemat.app (src-tauri/target/release/bundle/macos)
	npm run tauri build -- --bundles app

install: build ## Put Lemat.app in APP_DIR (~/Applications) and link `lemat` in BIN_DIR
	mkdir -p $(APP_DIR) $(BIN_DIR)
	rm -rf $(APP_DIR)/Lemat.app
	cp -R $(BUNDLE) $(APP_DIR)/Lemat.app
	ln -sf $(APP_DIR)/Lemat.app/Contents/MacOS/lemat $(BIN_DIR)/lemat

icon: ## Draw the app icon (scripts/make-icon.swift) and make every size of it
	mkdir -p build
	swift scripts/make-icon.swift build/icon.png
	npx tauri icon build/icon.png
	rm -rf src-tauri/icons/android src-tauri/icons/ios src-tauri/icons/64x64.png

run: deps lejce/Cargo.toml ## Run a development build (BOARD=file.md opens that file)
	npm run tauri dev $(if $(BOARD),-- -- -- "$(abspath $(BOARD))")

test: lejce/Cargo.toml ## Run the Rust tests and the web view tests
	cd src-tauri && cargo test
	node --test 'src/*.test.js'

format: ## Format the Rust code
	cd src-tauri && cargo fmt

lint: lejce/Cargo.toml ## Run clippy
	cd src-tauri && cargo clippy --all-targets -- -D warnings

# Lejce runs the agents. It is a git submodule: a clone without
# --recurse-submodules has an empty folder, so fetch it then.
lejce/Cargo.toml:
	git submodule update --init lejce

push: ## Push main to every remote in REMOTES (default origin)
	for r in $(REMOTES); do git push $$r main || exit 1; done

hooks: ## Install the git pre-commit hook that blocks private text
	ln -sf ../../scripts/check-private.sh .git/hooks/pre-commit

check-private: ## Check the staged changes for private text
	scripts/check-private.sh

clean: ## Remove build output and npm packages
	cd src-tauri && cargo clean
	rm -rf node_modules

-include Makefile.local
