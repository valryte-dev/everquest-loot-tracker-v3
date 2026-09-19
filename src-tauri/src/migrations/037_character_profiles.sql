CREATE TABLE IF NOT EXISTS character_profiles (
    character_name TEXT PRIMARY KEY COLLATE NOCASE,
    race_code TEXT NOT NULL DEFAULT 'hu',
    gender TEXT NOT NULL DEFAULT 'm' CHECK(gender IN ('m', 'f')),
    class_code TEXT NOT NULL DEFAULT '',
    level_override INTEGER CHECK(level_override BETWEEN 1 AND 255),
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT OR IGNORE INTO character_profiles(character_name, race_code, gender, class_code)
SELECT
    substr(key, length('character_model_profile_') + 1),
    CASE
        WHEN json_valid(value) AND json_extract(value, '$.race') IS NOT NULL
        THEN json_extract(value, '$.race')
        ELSE 'hu'
    END,
    CASE
        WHEN json_valid(value) AND json_extract(value, '$.gender') = 'f'
        THEN 'f'
        ELSE 'm'
    END,
    CASE
        WHEN json_valid(value) AND json_extract(value, '$.classCode') IS NOT NULL
        THEN json_extract(value, '$.classCode')
        ELSE ''
    END
FROM app_settings
WHERE key LIKE 'character_model_profile_%'
  AND length(key) > length('character_model_profile_');

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES (37, 'persistent character profiles and level overrides');
