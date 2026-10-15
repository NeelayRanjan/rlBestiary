CARGO := ./scripts/cargo
.PHONY: setup check test bench run dist longrun ecology-longrun evolution-longrun
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
	$(CARGO) bench -p bestiary-world --bench world --locked
	./scripts/bench-viewer
run:
	./scripts/run-demo
dist:
	$(CARGO) build --workspace --release --locked
	mkdir -p dist
	cp target/release/bestiary target/release/bestiary-daemon target/release/bestiary-viewer dist/
longrun:
	./scripts/longrun longrun

ecology-longrun:
	$(CARGO) test --release -p bestiary-world --test trophic --locked -- --ignored --nocapture --test-threads=1
evolution-longrun:
	./scripts/longrun evolution
