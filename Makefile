.PHONY: run check format format-check tests python-check
run:
	cargo run -p rf-workbench
check:
	cargo clippy --workspace --all-targets -- -D warnings
format:
	cargo fmt --all
	cd python && uv run ruff format .
format-check:
	cargo fmt --all -- --check
	cd python && uv run ruff format --check .
tests:
	cargo test --workspace --locked
	cd python && uv run pytest --cov --cov-branch
python-check:
	cd python && uv run ruff check .
	cd python && uv run ty check
	cd python && uv run basedpyright
