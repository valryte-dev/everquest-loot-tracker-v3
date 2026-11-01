ALTER TABLE experience_calibrations
ADD COLUMN scale REAL NOT NULL DEFAULT 1.0;

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES (41, 'experience numeric calibration');
