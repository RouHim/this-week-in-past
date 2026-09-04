-- V2 / 02-backfill_taken: idempotently populate taken from JSON where NULL
UPDATE resources
SET taken = json_extract(value, '$.taken')
WHERE taken IS NULL
  AND json_extract(value, '$.taken') IS NOT NULL;
