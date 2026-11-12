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

build: install ## Build the release bundle (src-tauri/target/release/bundle)
	npm run tauri build

run: install ## Run a development build
	npm run tauri dev

test: ## Run the Rust tests
	cd src-tauri && cargo test

format: ## Format the Rust code
	cd src-tauri && cargo fmt

lint: ## Run clippy
	cd src-tauri && cargo clippy --all-targets -- -D warnings

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
