CARGO := ./scripts/cargo
.PHONY: setup check test bench run dist longrun
setup:
	$(CARGO) fetch --locked
check:
	$(CARGO) fmt --all -- --check
	$(CARGO) clippy --workspace --all-targets --locked -- -D warnings
	$(CARGO) test --workspace --locked
	$(CARGO) build --workspace --release --locked
test:
	$(CARGO) test --workspace --locked
bench:
	$(CARGO) bench -p bestiary-sim --bench throughput --locked
run:
	$(CARGO) run -p bestiary-cli -- world new --seed 42 --out worlds/scratch.db
	$(CARGO) run -p bestiary-viewer -- worlds/scratch.db
dist:
	$(CARGO) build --workspace --release --locked
	mkdir -p dist
	cp target/release/bestiary target/release/bestiary-daemon target/release/bestiary-viewer dist/
longrun:
	$(CARGO) test --release -p bestiary-world --test longrun --locked -- --ignored --nocapture
