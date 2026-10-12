ALTER TABLE workspace_projects ADD COLUMN paths TEXT;

UPDATE schema_version SET version = 26 WHERE singleton_id = 1;
UPDATE app_metadata SET value = '26' WHERE key = 'schema_version';
