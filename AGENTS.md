# Repository Guidelines

## Project Overview

Single-binary Rust photo-frame app (`this-week-in-past`): indexes local images by EXIF date, stores metadata in SQLite, serves a `/api/*` + embedded vanilla-JS slideshow (`web-app/` compiled into the binary, no frontend build).

## Architecture & Data Flow

Async only at HTTP edge (actix-web); ingest/cache/geo are blocking/parallel inside.

- Ingest (sync, rayon-parallel): `scheduler::index_resources` → `ResourceReader::read_all` (`par_iter` over `RESOURCE_PATHS`) → `filesystem_client::read_files_recursive` + `fill_exif_data` → `store.add_resources` in one rusqlite transaction → `vacuum`.
- Query (async handlers, blocking DB): endpoint → `image_cache::get` (fs hit) else `fs::read` + `image_processor::adjust_image` + `image_cache::put`; SQLite reads via `r2d2` pool (sync methods on `ResourceStore { pool }`).
- Shared state: two `Clone` structs via `web::Data` (`ResourceStore`, `ResourceReader`); no DI framework — constructor fns (`resource_store::initialize`, `resource_reader::new`).
- Background: `clokwerk` daily job at 00:05 + immediate `thread::spawn` on boot (`src/scheduler.rs`).
- Geo (offline): GeoNames `cities500.txt` bulk-loaded into `rstar::RTree` behind `OnceLock` + `tokio::Mutex` single-flight, loaded in `web::block`; `k=20` haversine scan ≤50km, PPLX district → most-populous parent ≤30km as `District, City` (`src/geo_location.rs`).
- Blocking I/O (`ureq` weather, cities500 load) always in `web::block`; CPU scan uses `rayon`, never async tasks.
- Config is env-vars only (`src/config.rs` pattern: `env::var(..).unwrap_or(default)`); `RESOURCE_PATHS` panics if missing.

## Key Directories

- `src/`: all Rust code + co-located tests (no `tests/` dir, no `docs/`, no `scripts/`).
- `migrations/`: `01-initial` … `04-drop_geo_location_cache`, each `up.sql`; loaded via `rusqlite_migration` `from-directory` (`build.rs` sets `rerun-if-changed=migrations`).
- `web-app/`: `index.html`, `script.js`, `style.css`, `images/`, `fonts/` — vanilla JS, edited directly, embedded via `include_str!/include_bytes!` (`src/web_app_endpoint.rs`).
- `.github/workflows/scripts/`: `prep-build-env.sh`, `translate-arch-to-rust-tripple.sh`, `upload-asset-to-release.sh` (CI cross-build/upload helpers).
- `.container/stage-arch-bin.sh`: picks `target/<triple>/release/` binary matching `uname -m` for scratch image.

## Development Commands

```bash
cargo fmt --all -- --check
cargo clippy --all-targets
cargo +nightly rustdoc -- -Z unstable-options --check
CITIES500_PATH=/tmp/cities500.txt cargo test
cargo test <name_substring>          # e.g. cargo test week_image
RESOURCE_PATHS=~/Pictures DATA_FOLDER=./data cargo run
docker build -f Containerfile -t this-week-in-past .
docker compose up                    # ~/Pictures:/resources:ro, 8080:8080
```

One-time native geo setup (container bakes this to `/cities500.txt`):

```bash
curl -o cities500.zip https://download.geonames.org/export/dump/cities500.zip
unzip -p cities500.zip cities500.txt > cities500.txt
CITIES500_PATH=$PWD/cities500.txt cargo test
```

Key env vars (`README.md` table is source of truth): `RESOURCE_PATHS` (required, comma-separated), `DATA_FOLDER` (default `./data`, legacy `CACHE_DIR` fallback), `PORT` (default `8080`), `CITIES500_PATH` (default `/cities500.txt`), `SLIDESHOW_INTERVAL=30`, `REFRESH_INTERVAL=360`, `WEATHER_UNIT=metric`, `OPEN_WEATHER_MAP_API_KEY` + `WEATHER_LOCATION=Berlin`.

## Code Conventions & Common Patterns

For the higher-level engineering principles (SOLID, YAGNI, error surfacing) see `## Engineering Principles` below; this section covers the file-level mechanics.

### Rust

**File structure**: Flat single files (no nested `mod` directories). Each file has a single responsibility. Module declarations live in `main.rs`.

**Error handling**: Use `thiserror`-derived `AppError` enum. Every variant maps to an HTTP status code and JSON body `{"error": "..."}` via `IntoResponse`. No `unwrap`/`expect` in non-test code. Convert `sqlx::Error` with `#[from]`, `serde_json::Error` with a manual `From` impl.

**Async pattern**: Handlers are `async fn` returning `Result<Json<T>, AppError>` or `Result<(StatusCode, Json<T>), AppError>`. DB access uses `SqlitePool` with `sqlx::query!` / `sqlx::query_as!` macros — no manual locking needed.

**State injection**: Axum `State(Arc<AppState>)` extractor. `AppState` holds a `SqlitePool`.

**Logging**: `tracing` with `#[instrument(skip(state))]` on handlers. `tracing-subscriber` with `EnvFilter` (default `info`, overridable via `RUST_LOG`).

**Validation**: `db::validate_meal()` enforces: name 1–200 chars, instructions 1–20000 chars, 1–100 ingredient lines (name ≤100 chars, quantity ≤50 chars), portions 1–10000. Both backend and frontend enforce the same limits. Validation runs inside `insert_meal` and `update_meal` before touching the DB.

**Testing**:
- DB tests: `#[tokio::test]` (async) for operations touching the DB, `#[test]` for pure validation/string helpers. Use `tempfile::TempDir` for isolated databases.
- Route tests: `#[tokio::test]`, use `tower::ServiceExt::oneshot` to send `Request` objects to the router. Helper `TestCtx` struct holds the app and temp directory.
- Naming convention: `given_<precondition>_when_<action>_then_<expected_result>`.

**Dependencies**:
- Keep external crates as low as possible; prefer `std` and built-in features (e.g., `sqlx`'s `SqlitePool` is already in the tree — use its connection pool, not a standalone connection manager).
- Before adding a third-party crate, evaluate the trade-off: maintenance burden, transitive deps, MSRV impact, and whether `std` or an existing dep already covers it.
- Pin the very latest stable version available on crates.io at the time of introduction; bump via `cargo upgrade` and review changelogs for breaking changes.
- Current set: `axum` (http1+json+query+multipart+macros), `sqlx` (sqlite+runtime-tokio+chrono+migrate+macros), `serde`/`serde_json`, `chrono`, `tokio`, `tracing`/`tracing-subscriber`, `thiserror`, `rust-embed`, `reqwest` (rustls+json+form), `image` (jpeg+png+gif+bmp+webp), `scraper`, `recipe-scraper`, `genai`, `base64`, `rand`, `ammonia`, `zip`. Dev: `tempfile`, `tower` (util only).

## Engineering Principles

### SOLID

One-line summary: each principle maps directly to a project convention — follow them to keep the codebase maintainable and testable.
- **Single Responsibility**: one file = one concern (already encoded in the file-structure rule above).
- **Open/Closed**: extend behavior by adding new route handlers or DB functions rather than mutating existing ones whose tests are green.
- **Liskov Substitution**: handlers and DB functions are called through their concrete return types; no subtype-swap tricks. Listed for completeness — not a current concern.
- **Interface Segregation**: prefer narrow function signatures over fat `AppState` accessors; pass only the needed value (e.g., a `&SqlitePool` reference) into helpers.
- **Dependency Inversion**: handlers depend on `AppState` (a concrete struct); DB-free logic lives in pure functions that take their inputs by value, keeping them testable without a DB.

### YAGNI

Don't add functionality, configuration knobs, abstractions, or `mod` directories until a concrete caller needs them.
If a function has no call site, delete it — no commented-out scaffolds, no `#[allow(dead_code)]` to keep a future placeholder.

### UX & Error Surfacing

- Handle user interactions gracefully: every HTTP error path returns a structured JSON `{"error": "..."}` body with the correct status code; the frontend renders it inline — never an uncaught `unwrap` panic surfaces a 500 with no body.
- Surface actionable errors to the UI: messages name the offending field and the constraint (e.g., `name must be 1–200 characters`), not a bare `invalid input`.
- Validation runs in `db::validate_meal` before any DB write; the frontend calls the same `validateMeal()` helper from `$lib/validation.ts` to mirror the constraint.

## Workflow Practices

1. **Web search second opinion**: when planning a non-trivial feature, do not rely solely on training data — run a web search (or `mcp__context_query_docs` for libraries) to confirm current best practices, latest API shape, and any deprecations since the model's cutoff. Record the source in the plan's Research Notes if it changes the design.
2. **Lint before done**: CI enforces these gates on every push/PR: `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cd web && npm run check`. Run them locally before opening a PR to avoid round-trips.
3. **Consistent style**: match the existing file's formatting (4-space indent, no trailing whitespace, `rustfmt` defaults), so a reviewer can read the diff rather than the new file.

## Runtime/Tooling Preferences

- Toolchain: `cargo`, no `rust-toolchain*` / `.cargo/config` — CI pins stable (`minimal` + rustfmt/clippy) and nightly (rustdoc/udeps) via `actions-rs/toolchain@v1` + `Swatinem/rust-cache@v2`.
- Package manager: `cargo`/`crates.io`; npm only for CI `semantic-release` (Node 24, main branch only).
- Cross-build: `source .github/workflows/scripts/prep-build-env.sh && build-rust-static-bin <x86_64-musl|aarch64-musl|armv7-musleabihf|arm-musleabihf>` (`messense/rust-musl-cross` docker).
- Runtime: fully static musl on `scratch` (no glibc); `mimalloc` musl-only; release `panic=abort`, `lto=true`, `codegen-units=1`, `strip=true`. Container needs writable `DATA_FOLDER` (`/data` volume) and read-only `/resources` mount; native run needs `CITIES500_PATH` or geo resolves to `None` with warning.

## Testing & QA

Follow **TDD**: write the failing test first, watch it fail, then implement the smallest change to make it pass; refactor only after green. This applies to both Rust (`#[cfg(test)]` and route integration tests) and the frontend (`*.test.ts`).
All tests are written in **BDD** style: name them by behavior, not implementation. Rust: `given_<precondition>_when_<action>_then_<expected_result>`. Frontend: `describe('<unit>', () => { it('<observable behavior>', ...) })`. BDD names double as living documentation.

### Rust tests (`cargo test`)

- **Location**: `#[cfg(test)]` modules — inline within each source file, or in flat sibling `*_tests.rs` modules declared in `main.rs` (e.g. `routes_tests.rs`, `db_tests.rs`).
- **DB layer** (`db.rs`): Unit tests for CRUD, validation edge cases (empty strings, boundary lengths, whitespace-only), search filtering, week math, weighted meal selection, ingredient aggregation.
- **Route layer** (`routes.rs`): Integration tests using `tower::ServiceExt::oneshot`. Verify status codes, response bodies, search filtering, 404 for missing resources, plan CRUD, import endpoints, Bring! handlers.
- **Plan, import, seed, image** (`plan.rs`, `import.rs`, `seed.rs`, `image.rs`): Unit tests for week math, weighted selection determinism, seeding idempotency, image conversion.
- **Error layer** (`error.rs`): Verify each `AppError` variant maps to correct status code and JSON body.
- **Model layer** (`model.rs`): Serde round-trip and field deserialization.
- **Static assets** (`static_assets.rs`): Verify SPA fallback returns index.html, correct MIME types.
- **Isolation**: Every test uses `tempfile::TempDir` for fresh databases. No shared state between tests.
- **TDD workflow**: for any new DB function or route handler, add the failing test inside the same `#[cfg(test)] mod tests` block, run `cargo test -- <test_name>`, watch red, then implement.
- **No unwrap/expect in non-test code** is the production rule; tests may use them freely, but prefer `assert_eq!` / `assert!` with a failure message naming the precondition.
