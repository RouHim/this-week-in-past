-- Resource ids change from md5(file_name) to md5(full path) in this release.
-- Hidden entries keyed to the old ids are stale: reset the selection once so
-- no orphaned old-format ids remain in the hidden list after the upgrade.
DELETE FROM hidden;
