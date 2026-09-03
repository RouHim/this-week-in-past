# Feature Specification: District-aware hierarchical city display (Issue #209)

**Created**: 2026-09-03
**Status**: Draft (pending approval)
**Input**: https://github.com/RouHim/this-week-in-past/issues/209
**Related**: Offline `cities500` lookup (`src/geo_location.rs`, `resolve_city_name`), `resource_processor::build_display_value`
**GeoNames**: `cities500.zip` ~185k entries, `readme.txt` geoname table (19 columns), feature codes http://www.geonames.org/export/codes.html

## Goal
When a photo’s GPS resolves to a **district / section of a populated place** (e.g. *Bayenthal* → Köln, *Volksdorf* → Hamburg, *Christianshavn* → København) the UI must show a hierarchical, unambiguous label instead of the bare district name: `District, City` (e.g. `Bayenthal, Köln`, `Christianshavn, København`). Plain city photos keep the current single-name behavior (`Amsterdam`, `Köln`).

Single-file SQLite, WAL, `r2d2` pool=10, scratch+static-musl, `<50 MB` heap/RSS delta, and offline-only (no API key, no network) constraints are preserved. No new environment variable; existing `CITIES500_PATH` / `BIGDATA_CLOUD_API_KEY` handling unchanged.

## User Scenarios

### Scenario 1 — District photo (P1)
A user opens a photo taken in Bayenthal (50.9049, 6.9606).

**Acceptance**
1. Given district coordinate within `MAX_DISTANCE_KM=50` of *Bayenthal* (feature `PPLX`, country `DE`), when `resolve_city_name` runs, then the nearest parent city within `MAX_PARENT_DISTANCE_KM=30` with feature `PPL|PPLA*|PPLC` is found (`Köln`) and the resolved string is `Bayenthal, Köln`.
2. Given `build_display_value` renders, then display is `15.03.2021, Bayenthal, Köln` (not `..., Bayenthal`).
3. Given a Volksdorf coordinate (53.651, 10.166), then display is `Volksdorf, Hamburg`.

### Scenario 2 — Foreign district (P1)
A user opens a photo taken in Christianshavn (55.676, 12.593, `DK`).

**Acceptance**
1. Given resolved `Christianshavn`, then display is `Christianshavn, København` (two-level hierarchical, no country suffix).
2. Given Amsterdam at 52.37403, 4.88969 (plain city, not district), then display is `Amsterdam` (single name).

### Scenario 3 — Plain city (P2)
Photo near Köln Dom (50.941, 6.958) resolves to `Köln` (feature `PPLA`, not `PPLX`).

**Acceptance**
1. Given coordinate resolves to plain city `Köln`, then display is `Köln` (no parent, no country).

### Scenario 4 — No parent found / ocean (P2)
Coordinate maps to a `PPLX` entry but no parent candidate within 30 km (e.g. isolated `PPLX` on island) or ocean/desert >50 km.

**Acceptance**
1. Then display falls back to district name alone (`Bayenthal`) — never panics, never picks a far-away parent.
2. Ocean/desert >50 km → `None` → `build_display_value` shows date only (unchanged).

## Functional Requirements

- **FR-001 — Extended CityEntry**: `load_city_index()` must parse GeoNames `geoname` columns beyond `name/lat/lon`: `feature_class` (col 6), `feature_code` (col 7), `country_code` (col 8), `admin1_code` (col 10), `population` (col 14). Store in `CityEntry { name, lat, lon, feature_code, country_code, admin1_code, population }`. Malformed lines log `warn!` and skip (existing behavior). File size/line validation in `Containerfile` stays `>100k lines, >5 MB`.

- **FR-002 — Subdivision detection**: An entry is a *district/subdivision* iff `feature_class == "P" && feature_code == "PPLX"` (section of populated place). `STLMT`, `PPLL`, `PPLQ` etc. are **not** treated as districts in v1; they render as plain names. Rationale: `cities500` filters `P` down to `PPLA4`; `PPLX` alone covers the reported cases (Bayenthal, Volksdorf, Christianshavn). Keep single predicate for reviewability; extend via data-driven list later if needed.

- **FR-003 — Parent resolution**: Maintain two `RTree`s **or** one tree with a filtered view: `full_tree` (all entries) for initial nearest, `parent_tree` (entries where `feature_code in {"PPL","PPLA","PPLA2","PPLA3","PPLA4","PPLC","PPLG","PPLS"}`) for parent lookup. When `best` from `full_tree` (existing 20-nearest haversine scan + antimeridian probe) is a `PPLX`:
  1. Scan `parent_tree.nearest_neighbor_iter(&query_point).take(100)` for each of `point` and `alt_point` (same Euclidean `distance_2` / `AABB` envelope contract; increased from 20 to 100 to collect denser urban candidates, deduped by `name+lat+lon` to avoid double counting from the antimeridian probe).
  2. Collect all candidates with `haversine_km` from the **photo coordinate** (not district centroid) `<= MAX_PARENT_DISTANCE_KM = 30.0`, then pick the **most populous** parent — `population` descending, `haversine_km` ascending as tie-breaker — within 30 km.
  3. If none within 30 km, parent = `None`.
  Parent scan reuses the existing `haversine_km` clamp and antimeridian `lon±360` probe. Rationale (see `src/geo_location.rs::resolve_city_name`): population heuristic matches product expectations — e.g. Volksdorf (53.651, 10.166) prefers Hamburg (~1.8M, ~12–16 km) over the geographically closer Ahrensburg (~6 km, 33k); Bayenthal→Köln ties are broken by distance. Pure closest-haversine would select Ahrensburg and break Scenario 1, so FR-003 intentionally diverges from a minimum-distance rule.

- **FR-004 — Display formatting** (`geo_location::resolve_city_name` returns the *display string*, not raw name):
  ```
  if district && parent.is_some() { format!("{}, {}", district.name, parent.name) }
  else if !district { district.name } // actually city name
  else { district.name } // fallback, no parent
  ```
  `build_display_value` is unchanged (it already does `", {city_name}"`). No second geocoding pass.
- **FR-005 — No country suffix**: No country is appended. Display is strictly `District, City` or `City`. The earlier `HOME_COUNTRY` / three-level idea is intentionally dropped per review 2026-09-03. Country code is parsed but not rendered.

- **FR-006 — Remove persistent geo cache (Option 2, 2026-09-03)**: The offline `RTree` lookup is `<1ms` (20-nearest haversine) — no network/rate-limit anymore. Persistent `geo_location_cache` (`resources.db` table) is removed entirely. `resource_processor::get_city_name` becomes a thin `geo_location::resolve_city_name(loc).await` passthrough (no `location_exists`/`get_location`/`add_location`). A migration `04-drop_geo_location_cache/up.sql` with `DROP TABLE IF EXISTS geo_location_cache` deletes the table on next startup. Operators need no manual `DELETE`; stale `Bayenthal` rows vanish with the table. No replacement cache; an optional volatile `HashMap` per scheduler run could be added later if burst-photo profiling shows need, but not in v1.

- **FR-007 — Configuration & deprecation**: `BIGDATA_CLOUD_API_KEY` stays deprecated/ignored. `CITIES500_PATH` unchanged. No new env var. `geo_location_cache` removal is breaking at DB level but invisible to API (same `display_value` string, just computed live).

- **FR-008 — Performance & resource bounds**: `load_city_index` via `web::block` still non-blocking on actix workers; load time <1 s for 185k rows, heap <20 MB for entries + ~15 MB for R*-tree nodes, total <50 MB. `resolve_city_name` stays `async fn → Option<String>`; parent lookup adds at most one extra 100-per-probe haversine scan (up to ~200 trig with antimeridian probe, deduped) — p95 <1 ms. No new crate with nightly or large transitive deps. Removing the SQLite cache actually reduces WAL writes and DB size.

- **FR-009 — Observability**: `load_city_index` `info!` logs `loaded N cities (M parents, K districts)`; district→parent resolutions `debug!` with `district, parent, distance_km`. Existing `warn!` for malformed lines retained. Migration `04` logs via `rusqlite_migration` `info!` on apply.

- **FR-010 — Testing**: Keep existing `resolve_koblenz/amsterdam/kottenheim/negative_dms/invalid/mid_ocean` tests green (update any that seeded `geo_location_cache` to not expect caching). Add new tests:
  - `resolve_bayenthal_returns_hierarchical` (approx Bayenthal 50.904, 6.960 → `Bayenthal, Köln` when present; dataset-tolerant: assert `contains "Köln"` and if `contains "Bayenthal"` then exact `Bayenthal, Köln`).
  - `resolve_plain_city_unchanged` (Köln Dom → `Köln`).
  - `resolve_christianshavn_hierarchical` (55.676, 12.593 → `Christianshavn, København` tolerant: `contains København`).
  - `parent_out_of_range_fallback` — synthetic `CityEntry` with lone `PPLX` far from any parent, assert fallback to district alone.
  - Migration test: `geo_location_cache` dropped after `04` (in-memory `MIGRATIONS.validate()` and on-disk `user_version` bump).
  Tests run offline against baked `cities500.txt` (`CITIES500_PATH`).
## Key Entities
- **CityEntry**: `{ name, lat, lon, feature_code, country_code, admin1_code, population }` stored in `RTree<CityEntry>` (`rstar 0.12`). `Envelope = AABB<[lon,lat]>`, `PointDistance::distance_2` Euclidean (rstar contract), haversine selects true nearest.
- **CityIndex**: `{ full_tree: RTree<CityEntry>, parent_tree: RTree<CityEntry> }` behind `OnceLock<Option<CityIndex>>` / `CITY_INDEX`.
- **DisplayString**: `String` returned by `resolve_city_name` directly used by `build_display_value` (`"{date}, {DisplayString}"`). No persistent cache.

## Edge Cases
- Ocean / desert >50 km from any entry → `None` → `build_display_value` shows date only (unchanged).
- Antimeridian: district near 180° — both `lon` and `lon±360` probes apply to parent scan as well.
- Duplicate names: Hamburg has `Hamburg` and `Hamburg-Nord` etc.; parent scan picks the most populous candidate within 30 km (population desc, haversine asc tie-breaker), not the geographically closest nor lexicographic.
- Invalid/future GeoNames columns: `cols.len() < 15` → column 14 missing → population defaults 0, not fatal; feature/country empty → treat as plain city.
- Concurrent `ensure_city_index` callers: `OnceLock` + `web::block` single flight preserved.
- Migration `04` idempotent `DROP TABLE IF EXISTS geo_location_cache` — succeeds whether table present, already dropped, or fresh DB (where `01-initial` still creates it briefly before `04` drops it; final schema has no `geo_location_cache`).
- Missing dataset district (e.g. Volksdorf <500 pop not in `cities500`): nearest will be Hamburg itself → renders `Hamburg` (acceptable fallback, `contains`-tolerant).
- Huge panorama OOM guard and JPEG filesystem cache unrelated — untouched.
## Research Notes
- GeoNames `cities500.zip` ~10 MB uncompressed, ~185k rows, tab-delimited UTF-8. Columns: `geonameid name asciiname alternatenames latitude longitude feature_class feature_code country_code cc2 admin1 admin2 admin3 admin4 population elevation dem timezone mod_date`. Verified via `https://download.geonames.org/export/dump/readme.txt` and `https://www.geonames.org/export/codes.html` — `P:PPLX` = “section of populated place” (district/neighborhood). `PPL*` family filtered for parents. Alternative `hierarchy.zip` provides parentId/childId but requires second file + ~10 MB and graph walk — rejected for Pi 512 MB.
- Offline `rstar 0.12` R*-tree: pure-Rust, no unsafe, musl static cross builds cleanly (chosen in prior decision over `kiddo` nightly). Euclidean `distance_2` contract satisfied; haversine post-filter within 50 km ensures true geo nearest (existing pattern).
- `MAX_DISTANCE_KM=50` already covers ocean case; `MAX_PARENT_DISTANCE_KM=30` chosen to cover Volksdorf→Hamburg (~12 km), Bayenthal→Köln (~4 km), Christianshavn→København (~2 km) while rejecting far spurious parents. Tuneable via const. Parent within 30 km is selected by most populous (`population` desc, haversine asc tie-breaker) per FR-003 — e.g. Volksdorf prefers Hamburg (1.8M) over nearer Ahrensburg (33k, ~6 km) — not pure closest haversine.
- Prior art: `bigdatacloud` API + `geo_location_cache` (persistent SQLite) kept `locality` alone and cached it. With offline `rstar` the cache is obsolete (`<1ms` vs `~0.5ms` SQLite) — Option 2 drops it entirely, reducing WAL writes. Env var for home country considered and dropped 2026-09-03.

## Assumptions
- `cities500` is sufficient; hamlets `<500` pop (without adm seat) are not displayed — by design (`<50 MB`, 99% city coverage). If a district’s pop `<500` and not a seat, it won’t appear — fallback is parent city.
- No new container layer or download; `geodata` stage still fetches only `cities500.zip`. No `alternateNames.zip` or `hierarchy.zip`.
- `PPLX`-only subdivision predicate is correct; if future data shows districts with `PPLQ` etc., predicate extends without schema change.
- Country code parsed but not displayed; can be reused later if 3-level is reintroduced.
- Dropping `geo_location_cache` is safe: no external consumer queries that table directly; `MIGRATIONS` `user_version` bump from 3→4 handles fresh vs migrated DBs idempotently.

## Success Criteria
- **SC-001 — District hierarchical**: Photo at Bayenthal (≈50.90,6.96) resolves to string containing `Köln` and, when `Bayenthal` present, to `Bayenthal, Köln` (fallback `Köln` if absent) when `CITIES500_PATH` present.
- **SC-002 — Foreign district hierarchical**: Christianshavn (≈55.67,12.59) resolves to string containing `København` and, when `Christianshavn` present, to `Christianshavn, København`; Amsterdam (52.374,4.889) → `Amsterdam`.
- **SC-003 — Plain city unchanged**: Köln Dom (50.94,6.95) → `Köln`.
- **SC-004 — Backwards compat & perf**: Existing 9 `resolve_*` tests pass; fresh index load <1 s; `resolve_city_name` p95 <1 ms extra; RSS delta <5 MB over baseline; `geo_location_cache` table absent after migration `04` (`SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='geo_location_cache'` =0) for both fresh and migrated DBs; concurrent 3-client load does not deadlock.
- **SC-005 — Docs**: `README.md` unchanged regarding env vars (no new var); `CHANGELOG.md` notes hierarchical `District, City` + `BREAKING` note that `geo_location_cache` is dropped automatically (no manual `DELETE` needed); `CITIES500_PATH` missing still disables resolution with `warn!` (no panic).
