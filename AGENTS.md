# Repository Guidelines

## Project Structure & Module Organization
- `src/`: Warp backend (config/resource/weather/web_app endpoints, processors, schedulers); tests live beside code in `*_test.rs` and `integration_test_*.rs`.
- `web-app/`: static frontend served by the binary (`index.html`, `script.js`, `style.css`, `images/`, `fonts/`).
- Root: Rust workspace files (`Cargo.toml`, `Cargo.lock`), container assets (`Containerfile`, `docker-compose.yaml`), app assets (`icon.png`, `CHANGELOG.md`); `target/` holds build output and stays untracked.
- Runtime data paths are set by env vars (`RESOURCE_PATHS`, `DATA_FOLDER`); keep large media outside the repo.

## Build, Test, and Development Commands
- `cargo build` / `cargo run`: compile or start the backend locally (set `RESOURCE_PATHS=/path/to/pictures DATA_FOLDER=./data`).
- `cargo test`: run unit + integration tests (async cases use `tokio`).
- `cargo fmt` then `cargo clippy --all-targets -- -D warnings`: format and lint; keep CI free of warnings.
- `docker build -t rouhim/this-week-in-past -f Containerfile .`: produce the scratch-based image from staged binaries.
- `docker-compose up --build`: run locally against a bind-mounted photo directory via `/resources:ro`.

## Coding Style & Naming Conventions
- Rust 2021 with 4-space indent; rely on `cargo fmt` for spacing and import order.
- Files/modules use `snake_case`; types/traits use `PascalCase`; constants use `SCREAMING_SNAKE_CASE`.
- Group imports std → external → crate; prefer explicit `use crate::…` paths.
- Keep functions small, return `Result<T, E>`, and add `log`/`env_logger` context for failures.
- Frontend: vanilla JS/CSS in `web-app`; keep IDs/classes descriptive and consistent with existing names.

## Testing Guidelines
- Mirror the GIVEN/WHEN/THEN comment style shown in `resource_processor_test.rs`.
- Place unit tests next to code with `#[cfg(test)]`; use `#[tokio::test]` for async integration-style tests in `integration_test_*`.
- Use `assertor`/`pretty_assertions` helpers; avoid random data where determinism matters.
- Add coverage when touching HTTP endpoints, resource processing, or new config/env branches.

## Commit & Pull Request Guidelines
- Follow Conventional Commits (`feat:`, `fix:`, `chore(deps):`, `chore(release): … [skip ci]` aligns with history).
- One focused topic per commit; rebase before opening a PR.
- PRs should state intent, commands/tests run, config/env changes, and include screenshots for UI updates.
- Link related issues/discussions and flag breaking changes or data migrations explicitly.

## Configuration & Security
- Keep secrets local: `OPEN_WEATHER_MAP_API_KEY`, `BIGDATA_CLOUD_API_KEY`, `HOME_ASSISTANT_API_TOKEN` must not be committed.
- Default mounts expect read-only `RESOURCE_PATHS` and writable `DATA_FOLDER`; verify permissions when running in Docker or on NAS shares.
