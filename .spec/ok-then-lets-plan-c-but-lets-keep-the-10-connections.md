# Feature Specification: Steady-State Memory Optimization — Plan C (Filesystem JPEG Cache, Keep 10 Connections)

**Created**: 2026-08-30
**Status**: Approved
**Input**: Ok then lets plan C but lets keep the 10 connections

## Goal
Reduce steady-state RAM consumption during slideshow operation (after indexing completes) for libraries up to 90–100k photos without degrading image delivery latency or slideshow continuity. Scope covers the serving path only: bounded filesystem image cache, indexed week query, and guarded image decode — the 10-connection r2d2 SQLite pool and default actix worker count are explicitly kept. No hard 100 MB target; the change must be a measurable best-effort reduction over the current ~400 MB baseline.

## User Scenarios
### Scenario 1 - Steady-state slideshow on a large library (P1)
A self-hoster with ~90k photos finishes midnight indexing and starts the slideshow. The server runs for hours serving resized images without unbounded memory growth.

**Acceptance**
1. Given indexing has finished and the server is serving the slideshow, when the cache has reached its cap, then subsequent image requests evict the least-recently-used entry before inserting the new one so cache size stays bounded.
2. Given the slideshow is running, when a cached image is requested again, then it is served from the filesystem cache without re-decoding the source file.

### Scenario 2 - Three concurrent browsers browsing the same library (P1)
Three clients (e.g., three TVs/tablets) request random resized images at the same time.

**Acceptance**
1. Given three concurrent clients request different resized images, when all three requests execute at the same time, then all three receive a valid image response (200) without a pool-timeout error when the pool size is 10.
2. Given the cache cap is reached while three clients insert concurrently, when eviction runs, then no duplicate eviction or orphaned file remains and cache size stays at or below the cap.

### Scenario 3 - Migration from an existing database with BLOB cache and JSON week query (P2)
An existing deployment has a `resources` table with `id` + `value` JSON and a `data_cache` BLOB table populated from a previous version.

**Acceptance**
1. Given an existing `resources.db` without a `taken` column, when the application starts, then it adds a `taken TEXT` column, creates an index on it, and backfills `taken` once from each row's JSON `taken` value without requiring manual migration.
2. Given an existing `data_cache` table with BLOBs, when the application starts with the new filesystem cache enabled, then it ignores or removes `data_cache` entries and thereafter serves and populates only the filesystem cache.

### Scenario 4 - Oversized source image does not exhaust RAM (P2)
A library contains a very large panorama (e.g., 12000×8000) that would require >96 MB to decode.

**Acceptance**
1. Given a request for a resource whose decoded pixel count exceeds the defined limit, when the server handles the request, then it rejects the request with an error status without allocating the full decoded buffer.

## Functional Requirements
- **FR-001**: System must store resized image cache entries as files under `DATA_FOLDER/cache` (derived from the same `DATA_FOLDER` used for `resources.db`) keyed by resource id and display dimensions, not as rows in a `data_cache` BLOB table.
- **FR-002**: System must bound the filesystem image cache by both count and total bytes, with caps of approximately 500 files or 1 GB (whichever is reached first), and evict the least-recently-used entry (by file modification time or equivalent recency tracking) before inserting a new entry when the cap is reached.
- **FR-003**: System must encode every cached resized image as JPEG at quality 90 and serve it with a JPEG content type, regardless of the source image format (including PNG sources with or without alpha).
- **FR-004**: System must always decode the source file and re-encode to JPEG for a cache miss, even when requested display dimensions are `0×0` or otherwise indicate no resize; it must not short-circuit by serving original file bytes.
- **FR-005**: System must add a denormalized `taken TEXT` column to the `resources` table and create an index on it (`idx_resources_taken` or equivalent), populate it from the JSON `taken` value at index time, and backfill it once on first startup for existing rows.
- **FR-006**: System must resolve week-based queries (this-week visible resources and their count) using the indexed `taken` column, not by scanning and parsing JSON with `json_each` over all rows.
- **FR-007**: System must keep the r2d2 SQLite connection pool maximum size at 10 and must not introduce an environment variable to change it.
- **FR-008**: System must keep the actix-web worker count at the framework default (number of CPUs) and must not pin it to a fixed small number.
- **FR-009**: System must enforce a decoded pixel limit that rejects images whose pixel count exceeds 50 megapixels or whose dimensions exceed 8000×8000, returning an error response without completing the decode.
- **FR-010**: System must ensure cache eviction and insertion are safe under concurrent access from up to 3 simultaneous clients, with no orphaned files or cap violations.
- **FR-011**: System must persist the filesystem cache across restarts (files remain under `DATA_FOLDER/cache` until evicted by LRU) and must create the cache directory if it does not exist.
- **FR-012**: System must treat an unreadable or missing cache directory entry as a cache miss and re-decode the source image, rather than failing the request.

## Key Entities
- **Resource**: Photo metadata record with `id`, `value` JSON (including `taken`, `path`), and denormalized `taken` column for indexed week queries.
- **ImageCacheEntry**: Filesystem file under `DATA_FOLDER/cache` representing a resized image for a specific `resource id` and display dimensions, encoded as JPEG quality 90, managed by LRU eviction.
- **ResourceStore**: SQLite-backed store that provides week-query, random-selection, and metadata lookup operations using the indexed `taken` column.

## Edge Cases
- Existing database without `taken` column or index: migration adds the column and index idempotently and backfills from JSON in a single pass.
- Existing database with populated `data_cache` BLOB table: new version ignores existing BLOBs and uses only filesystem cache; table may be dropped or left empty without affecting operation.
- `DATA_FOLDER/cache` directory missing or not writable at startup: system creates it; if creation fails, requests treat cache as disabled (decode on every request) and log a warning rather than crashing.
- Cache cap reached under concurrent inserts: only one eviction per insert, no double-eviction or orphan.
- Source image missing, unreadable, or corrupt: request returns an error status, no cache file is written, and no panic occurs.
- Source image exceeds pixel limit or cannot be decoded: request returns an error status without caching a partial result.
- Library with no `taken` dates: week query returns empty set without scanning JSON.
- Three clients request the same uncached image concurrently: at most one decode+write wins, others either wait or serve the just-written file — no corrupted JPEG.

## Research Notes
- https://sqlite.org/intern-v-extern-blob.html — External BLOBs (separate files) are faster and use less page-cache RAM than SQLite BLOBs for blobs larger than ~100 KB, supporting the filesystem cache choice.
- https://sqlite.org/fasterthanfs.html — SQLite is only faster than the filesystem for small blobs (~<100 KB) and with a small tuned page cache; photographic resized images at JPEG-90 typically exceed this, favoring filesystem storage for this workload.
- https://harutoolslab.com/en/articles/jpeg-vs-png.html — JPEG at quality 85–90 is visually lossless for photos and 60–80% smaller than PNG, justifying the always-JPEG cache at quality 90 for this slideshow use case.

## Assumptions
- No hard steady-state RSS target exists; the goal is meaningful reduction from the current ~400 MB baseline without performance regression.
- Steady-state is measured after indexing completes; transient indexing memory (scan of 90k files) is out of scope for this optimization.
- The 10-connection r2d2 pool stays unchanged by explicit request; it is not tuned or made configurable.
- Actix workers stay at the default (`num_cpus`); no worker-count tuning is performed.
- Filesystem cache caps of ~500 files or ~1 GB are acceptable defaults; they may be tuned later without a spec change if kept within the same order of magnitude.
- JPEG quality 90 is acceptable for all photographic content, including PNG sources regardless of alpha channel; no transparency preservation is required.
- The always-decode path (no serve-original shortcut) is an explicit product decision; future addition of a shortcut would be a separate change.
- Decoded pixel limit of 8000×8000 / 50 MP is acceptable as a guard; images exceeding it are considered unsupported for the slideshow rather than downsampled via a streaming decoder.
- `DATA_FOLDER` is the source of truth for both `resources.db` and the `cache` subdirectory; cache files share the same lifecycle and backup scope as the database folder.
- `BIGDATA_CLOUD_API_KEY`, `cities500` offline geo DB, and `rstar` index are unrelated to this change and remain as currently implemented.

## Success Criteria
- **SC-001**: With a representative library (≥10k photos) and the cache warmed by requesting 600 distinct resized images, filesystem cache size stays at or below 500 files and 1 GB, with at least 100 evictions observed and no cap violation.
- **SC-002**: All cached resized images served after a cache miss are JPEG (magic bytes `FF D8`, Content-Type `image/jpeg`) and decode to dimensions that fit within the requested display bounds while preserving aspect ratio.
- **SC-003**: Week-based resource queries (`get_resources_this_week_visible_random` and count) complete without invoking `json_each` on the `resources.value` column, verified by query plan or by observing index usage on `taken`, and return the same result set as the pre-migration JSON scan for a library with 90k rows.
- **SC-004**: Three concurrent clients each requesting 20 distinct resized images (60 total requests issued simultaneously) all receive 200 responses with valid JPEG bodies and no pool-timeout or file-corruption errors when the pool size is 10.
- **SC-005**: Requesting a resized image for a source that would decode to >50 MP (or >8000 in either dimension) returns an error status within 2 seconds and does not increase RSS by more than 100 MB during the attempt, verified by process memory sampling.
- **SC-006**: Starting from a pre-migration `resources.db` with only `id`+`value` and a populated `data_cache` BLOB table, a single startup backfills `taken` for all rows, creates the index, and thereafter serves cache hits from the filesystem without reading `data_cache`.
