# Lemat: a math board that works with a coding agent.
#
# Machine-specific values (the git remotes to push to) go in Makefile.local,
# which git ignores. See Makefile.local.example.

.PHONY: help install build run test format lint push hooks check-private clean
.DEFAULT_GOAL := help

REMOTES ?= origin

help: ## Show this help
	@grep -hE '^[a-zA-Z0-9_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort -t: -k1,1 | \
		awk 'BEGIN {FS = ":.*?## "; w = 12} { k[NR] = $$1; v[NR] = $$2; if (length($$1) > w) w = length($$1) } \
		     END { for (i = 1; i <= NR; i++) printf "  \033[36m%-*s\033[0m  %s\n", w, k[i], v[i] }'

install: ## Install the Tauri CLI and other npm dependencies
	npm install

build: install lejce/Cargo.toml ## Build the release bundle (src-tauri/target/release/bundle)
	npm run tauri build

run: install lejce/Cargo.toml ## Run a development build (BOARD=file.md opens that file)
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
