-- V1 / 01-initial: baseline schema, idempotent for both fresh and existing DBs with user_version=0
CREATE TABLE IF NOT EXISTS hidden (
    id TEXT PRIMARY KEY
);

CREATE TABLE IF NOT EXISTS resources (
    id TEXT PRIMARY KEY,
    value TEXT,
    taken TEXT
);

CREATE TABLE IF NOT EXISTS geo_location_cache (
    id TEXT PRIMARY KEY,
    value TEXT
);

CREATE INDEX IF NOT EXISTS idx_resources_taken ON resources(taken);
