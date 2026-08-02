# Tech Debt Audit — this-week-in-past

Audit run: 2026-08-02 · Depth: standard · Status: first run (no prior report)

## Executive summary

- **Fixed this run (7 findings):** indexing panics on unreadable files, 3 clippy warnings, unused dev-dependency + dead code, 500→404 on missing resource metadata, brittle byte-count test, blocking HTTP inside async handlers, and 5 synchronous XHRs on the frontend main thread.
- **Biggest remaining:** resource IDs are `md5(file_name)` (`src/filesystem_client.rs:198`) — same-named files in different folders silently collide and one replaces the other. Needs a deliberate id-scheme decision + migration; not fixable as a mechanical cleanup.
- **`spec.md` (approved 2026-06-13, memory-optimization) is untracked and unimplemented.** The code has no size-bounded LRU cache, no reduced-resolution decode, no streaming indexing. Maintainer confirmed superseded/abandoned — recommend deleting or re-scoping the file.
- **Verification gates** (from CI, `.github/workflows/build-image.yaml`): `cargo fmt --all -- --check`, `cargo clippy`, `cargo test`. All green (test suite needs the two API-key secrets that CI injects — see below).
- Per maintainer decision: no `AGENTS.md` was created this run; learnings are in the "Learnings" section of this report instead.

## Architectural mental model

Actix-web HTTP server (Rust 2021, `src/main.rs`) serving a single-page slideshow (`web-app/`). At startup and at midnight (`src/scheduler.rs`) a background thread walks the configured `RESOURCE_PATHS`, extracts EXIF metadata per file (date taken, GPS, orientation), and writes id→JSON rows into a SQLite database (`resources` table). The slideshow frontend asks for this-week resource ids, then requests each photo as `/api/resources/{id}/{w}/{h}`; the handler decodes the source file fully, applies EXIF rotation, aspect-preserves it into the display bounds, encodes PNG, and stores the result in an unbounded `data_cache` BLOB table. Weather/geo city-name lookups hit external HTTPS APIs on demand. Everything is configured through env vars; the Docker image is a scratch container with a musl build.

## Findings table

| ID | Category | File:Line | Severity | Effort | Status | Description | Recommendation |
|---|---|---|---|---|---|---|---|
| FIX-1 | Error handling | `src/filesystem_client.rs:20,31,48,96,113,122,156,163,171,219` | High | S | **FIXED** | 8 panic sites in the indexing path; one unreadable file killed the background index thread silently (stale/empty store until midnight retry) | Log-and-skip each unreadable entry instead of panicking |
| QW-2 | Hygiene | `src/image_processor.rs:35`, `src/integration_test_resources_api.rs:10`, `src/integration_test_config_api.rs:77`, `src/integration_test_weather_api.rs:102` | Low | S | **FIXED** | 4 clippy warnings (`unnecessary_unwrap`, unused import, 2× `useless_borrows_in_formatting`) | Fixed |
| QW-1 | Dead code/deps | `Cargo.toml:37`, `src/resource_reader.rs:29`, `src/main.rs:1` | Low | S | **FIXED** | `pretty_assertions` dev-dep never imported; `ImageResource::with_taken_date` 0 call sites; `extern crate core` noise | Removed |
| QW-3 | Contract | `src/resource_endpoint.rs:202,223` | Med | S | **FIXED** | Missing resource returned 500 where sibling endpoints return 404 | Return `NotFound` |
| QW-4 | Test debt | `src/integration_test_resources_api.rs:336` | Med | S | **FIXED** | `assert_that!(response.len()).is_equal_to(316)` pinned exact PNG encoder output; breaks on image-crate bumps | Structural assertion (PNG magic + fits-in-bounds) |
| ARCH-1 | Async/blocking | `src/weather_processor.rs:25,56`, `src/geo_location.rs:150` | Med | M | **FIXED** | Blocking `ureq::call()` on actix worker threads, no timeouts | `web::block` + 15 s global timeout |
| FRONT-1 | Frontend perf | `web-app/script.js:52,237,345,365` (+ per-tick config refetch) | Med | M | **FIXED** | 5 synchronous XHRs blocked the main thread; preload config was re-fetched on every slideshow tick | One-time async config load at startup; all consumers read the cache |
| ID-1 | Correctness | `src/filesystem_client.rs:198` (id), `src/scheduler.rs:52` (HashMap dedupe), `src/resource_store.rs:232` (`INSERT OR REPLACE`) | High | L | NEW | Resource id = `md5(file_name)` → same-named files in different folders collide; one silently replaces the other; hide affects all copies | Deliberate id-scheme decision (path-based id + migration of `resources`/`hidden`/`data_cache`) — see Top 5 |
| SPEC-1 | Doc drift | `spec.md` (untracked, approved 2026-06-13) vs `src/resource_store.rs:354`, `src/image_processor.rs:35`, `src/scheduler.rs:52` | High | L | RESOLVED | Approved memory-optimization spec never implemented; code has no bounded cache/LRU, no reduced-resolution decode, no streaming indexing | Maintainer confirmed superseded; delete or re-scope the file |
| PERF-1 | Performance | `src/image_processor.rs:70,35` (PNG transcode/full decode), `src/resource_store.rs:354` (unbounded cache) | Med | L | NEW | Photographic sources re-encoded as PNG (3–10× larger than JPEG), decoded at full resolution, cached without bound | Revisit when SPEC-1 is decided; FR-002/FR-005 describe the target |
| ERR-2 | Error handling | `src/resource_store.rs:128,248` (`"failed:n"` typo, panic on DB error in request path), `:423` (query errors silently truncate results) | Med | M | NEW | All DB ops `panic!`/`unwrap`; a DB failure inside a request handler kills the request; `execute_query` silently returns partial lists on SQL errors | Return `Result` from store methods used by handlers; fix the typo; log-and-stop on query error |
| PERF-4 | Performance | `src/resource_store.rs:63,94` (json_each full scan over every resource per week query) | Med | L | NEW | Week/count queries JSON-parse every row with `json_each` — O(N) per slideshow refresh for 90k+ libraries | Denormalize `taken` into a column + index; part of the SPEC-1 territory |
| CHG-1 | Docs | `.releaserc` (`chore` → patch) + 1305-line `CHANGELOG.md` of near-empty stanzas | Low | S | NEW | Renovate dep bumps generate empty release entries | Group dep bumps with `skip ci` or restrict `chore` release rule; cosmetic |
| — | Test infra | `src/resource_processor_test.rs`, `src/integration_test_weather_api.rs` | Low | — | REJECTED | 6 tests require live API keys + internet (`BIGDATA_CLOUD_API_KEY`, `OPEN_WEATHER_MAP_API_KEY`) | Deliberate: CI injects both secrets (`build-image.yaml` test job). Local `cargo test` without keys fails these 6 — documented, not a bug |
| — | Deps | `kamadak-exif` (cargo machete) | — | — | REJECTED | machete flags it unused; false positive — crate lib name is `exif` (`src/exif_reader.rs:2` uses `exif::Exif`) | Ignore |

## Fixed this run

**FIX-1 — indexing no longer panics on unreadable files.** Replaced 8 `panic!` sites in `src/filesystem_client.rs` with log-and-skip: metadata/read_dir failures on folders (`:20,31,48`), non-UTF8 folder names (`:96`), ignore-marker scan failures (`:113,122`), unreadable files and metadata during resource reads (`:156,163,171`), and exif-open failures (`:219`, returns the resource without exif metadata). Tests added in `src/resource_reader_test.rs`: `read_dir_with_unreadable_file_does_not_panic` (chmod-000 file, skipped when euid 0) and `fill_exif_data_missing_file_does_not_panic`. Both failed on the pre-fix code (red), pass after. Follow-up this run: non-UTF8 file names and unreadable mtimes are now skipped instead of panicking (same let-else/log-and-skip pattern, `:156` and the `modified()` fallback at `:206`), and the per-entry skip logs were raised `debug!` → `warn!` so silently dropped files are visible at the shipped `RUST_LOG=info`. Regression test `read_dir_with_non_utf8_file_name_does_not_panic` (non-UTF8 name via `OsString::from_vec`, skipped when euid 0) panicked on the pre-fix code, passes after.

**QW-1/QW-2 — hygiene.** Removed `pretty_assertions` dev-dep (`Cargo.toml`, 23 lines pruned from `Cargo.lock`), dead `with_taken_date` (`src/resource_reader.rs`), `extern crate core` (`src/main.rs`); fixed `unnecessary_unwrap` (`src/image_processor.rs:27,35`), unused `FallibleIterator` import, and two `useless_borrows_in_formatting` (`src/integration_test_config_api.rs`, `src/integration_test_weather_api.rs`). Clippy: 4 warnings → 0.

**QW-3 — correct status codes.** `get_resource_metadata_by_id` and `get_resource_metadata_description_by_id` now return 404 for unknown ids (`src/resource_endpoint.rs:202,223`), matching `get_resource_by_id_and_resolution`. Regression test `test_get_unknown_resource_metadata_returns_not_found` (`src/integration_test_resources_api.rs:428`) pins 404 for unknown ids on both endpoints.

**QW-4 — robust image test.** Replaced the exact byte count with: PNG magic byte + decode header dimensions fit within the requested bounds and are non-degenerate (`src/integration_test_resources_api.rs:336`). This surfaced that `adjust_image` is *aspect-preserving* (`DynamicImage::resize`, `image-0.25.10/src/images/dynimage.rs:873`), which is correct for the `object-fit: contain` render path.

**ARCH-1 — no blocking HTTP on worker threads.** `get_current_weather`, `get_home_assistant_data` (`src/weather_processor.rs`) and `resolve_city_name` (`src/geo_location.rs`) now run their `ureq` calls inside `actix_web::web::block` with a 15 s global timeout (`ureq::RequestBuilder::config().timeout_global`).

**FRONT-1 — sync XHRs removed.** `web-app/script.js`: new one-time `loadAppConfig()` (async fetch, URL params take precedence) cached in `appConfig`; deleted `shouldOnlyPlayRandom`, `shouldPreloadImages`, `getSlideshowInterval`, `getRefreshInterval`; `getCurrentTemperatureDataFromHomeAssistant` is now async; removed the per-tick preload-config refetch. 5 sync XHRs → 0.

**Verification evidence**

```
cargo fmt --all -- --check   → clean
cargo clippy --all-targets   → 0 warnings
cargo test                   → 27 passed; 6 failed (all pre-existing, key-gated:
                               4× resolve_* need BIGDATA_CLOUD_API_KEY,
                               test_get_weather_current needs OPEN_WEATHER_MAP_API_KEY,
                               description test needs geo key — CI injects both;
                               all 33 pass with both keys exported)
node --check web-app/script.js → OK
```

Live smoke (debug binary, 2-image library): `/api/health` 200; indexing completed (2 resources); image endpoint 200, PNG 10×7; missing metadata → 404; hide/unhide 200/200. Browser (headless Chromium): page loads with zero console errors, image renders through the async config → playlist → image chain, `?SHOW_HIDE_BUTTON=true` URL override honored.

## Top 5 remaining

1. **ID-1 — resource id collisions (`src/filesystem_client.rs:198`).** Two `IMG_0001.jpg` in different folders → identical `md5(file_name)` → `HashMap` dedupe (`src/scheduler.rs:52`) + `INSERT OR REPLACE` (`src/resource_store.rs:232`) silently drop one; hiding one hides both. Impact grows with library size. Fix: hash the full path (or path+name), migrate persisted ids in `resources`, `hidden`, `data_cache` on startup, update tests that pin `md5(file_name)` (`src/resource_reader_test.rs:49,106,127`). Requires the maintainer to pick the scheme — it changes URL contracts and persisted data.

2. **SPEC-1 — decide the memory-optimization spec.** `spec.md` is approved but unimplemented and now untracked. Either delete it (superseded) or re-scope and plan it; currently it silently misdescribes the codebase to anyone who reads it.

3. **ERR-2 — store error handling (`src/resource_store.rs`).** Every DB operation panics or unwraps; request-path failures (e.g. `add_data_cache_entry` during image serving) crash the request. `execute_query` stops silently on SQL error, returning a partial list. Fix: `Result`-returning methods on the two request-path functions first, log-and-continue in `execute_query`, fix the `"failed:n"` typo at `:128,248`.

4. **PERF-1 — image pipeline (`src/image_processor.rs:35,70`, `src/resource_store.rs:354`).** Full-resolution decode → PNG re-encode → unbounded BLOB cache. This is the 400 MB RSS story the spec was written to fix; only actionable once SPEC-1's direction is settled.

5. **PERF-4 — week-query cost (`src/resource_store.rs:63,94`).** `json_each` full-scan + `strftime` comparison over every resource on each `/api/resources/week` request. Denormalize `taken` into a queryable column with an index once the schema is touched.

## Quick wins

- [ ] Delete or re-scope `spec.md` (untracked approved spec, confirmed superseded) — or `git add` it if it stays live.
- [ ] Fix `"failed:n"` → `"failed:\n"` panic messages (`src/resource_store.rs:128,248`).
- [ ] Add a `RESOURCE_PATHS`/`DATA_FOLDER` + test-keys note to `README.md` so `cargo test` is runnable locally (documented in CI only today).
- [ ] Investigate `localityLanguage=de` hardcode (`src/geo_location.rs:143`) — city names are always German regardless of `WEATHER_LANGUAGE`.
- [ ] Consider dropping the `chore`→patch release rule (`.releaserc`) or batching renovate bumps to stop empty CHANGELOG stanzas.

## Things that look bad but are actually fine

- **`kamadak-exif` "unused" per cargo machete** — false positive; lib name is `exif`.
- **Tests hit live network/external APIs** — deliberate; CI injects the two API keys, `resource_reader_test.rs` downloads fixtures from w3.org/github on every run. Flaky-by-design but an accepted convention here.
- **Aspect-preserving resize vs. requested exact dimensions** — `DynamicImage::resize` preserves aspect ratio; `#slideshow-image { object-fit: contain }` (`web-app/style.css`) makes that the correct contract. Not distortion.
- **Emoji-prefixed log messages** (`src/main.rs`, `src/scheduler.rs`) — consistent style across the repo, harmless.
- **`week/image` endpoint unused by the frontend** — plausible external/legacy client surface; not removed.
- **`CACHE_DIR` deprecated-but-honored** (`src/main.rs:66`) — documented in the code path and README; intentional migration path.
- **Panic on missing `RESOURCE_PATHS` / non-existent resource folder** (`src/main.rs:60`, `src/resource_reader.rs:110`) — fail-fast on misconfiguration, reasonable for a single-user self-hosted app.
- **API key in bigdatacloud query string** (`src/geo_location.rs:137`) — API-design requirement, HTTPS transport; not a leak vector in this codebase.

## Open questions for the maintainer

1. **spec.md** — delete, re-scope, or resurrect? (Marked superseded this run; file still on disk, untracked.)
2. **Resource id scheme** — is `md5(file_name)` intentional (stable ids across folder moves)? The collision behavior is almost certainly unintended for multi-folder libraries.
3. **`localityLanguage=de`** (`src/geo_location.rs:143`) — intentional hardcode or should it follow `WEATHER_LANGUAGE`?
4. **`week/image` endpoint** — still needed by any client?
5. **Local test workflow** — worth documenting the two API keys in the README so contributors can run the full suite locally?

## Not audited

- `.github/workflows/scripts/*` (prep-build-env, arch translation, asset upload) and `.container/stage-arch-bin.sh` — release plumbing, touched only by release automation.
- `web-app/index.html` and `web-app/style.css` beyond the slideshow-image/contain check — cosmetic surface.
- `docker-compose.yaml`, `renovate.json`, `Containerfile` — read, but no deep review of image layering or renovate config.
- Third-party API response handling (OpenWeatherMap/Home Assistant/bigdatacloud payload shapes) — exercised indirectly via tests/smoke only.
- No security tooling ran: `cargo audit`/Trivy run in CI (`.github/workflows/scheduled-security-audit.yaml`); not installed locally this run.

## Learnings (durable, for the next agent)

- `cargo test` fails 6 tests without `BIGDATA_CLOUD_API_KEY` and `OPEN_WEATHER_MAP_API_KEY` — that is the expected baseline, not a regression; CI injects both secrets.
- Resource ids are `md5(file_name)` and are load-bearing: they appear in URLs, the `hidden` table, and `data_cache` keys. Do not change the derivation without a migration.
- `adjust_image` is aspect-preserving by design and the frontend relies on it (`object-fit: contain`); "fixing" it to exact dimensions would distort photos.
- The image endpoint serves PNG regardless of source format; cache growth is unbounded — the (superseded) `spec.md` describes the intended bounded-JPEG design.
- Indexing must never `panic!` per-file: a single unreadable file previously killed the whole background index (fixed this run — keep it that way; skip-on-error is the pattern).
- The integration tests download fixtures from w3.org and raw.githubusercontent.com — they need internet.
