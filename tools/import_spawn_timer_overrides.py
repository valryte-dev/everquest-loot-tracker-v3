#!/usr/bin/env python3
"""Normalize nickname-keyed spawn data and generate the additive SQLite seed migration."""

import argparse
import json
from pathlib import Path


def sql(value):
    if value is None:
        return "NULL"
    if isinstance(value, (dict, list)):
        value = json.dumps(value, separators=(",", ":"))
    return "'" + str(value).replace("'", "''") + "'"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("source", type=Path)
    parser.add_argument("asset", type=Path)
    parser.add_argument("migration", type=Path)
    args = parser.parse_args()
    source = json.loads(args.source.read_text(encoding="utf-8-sig"))
    normalized = {}
    for nickname, raw in source.items():
        normalized[nickname] = {
            "name": raw["Name"],
            "shortName": raw.get("ShortName"),
            "zone": raw.get("Zone"),
            "continent": raw.get("Continent"),
            "lastKnownTod": raw.get("LastKnownTOD"),
            "respawnMinutes": int(raw["Respawn"]),
            "windowMinutes": int(raw.get("WindowLength") or 0),
            "windows": raw.get("Windows"),
        }
    args.asset.parent.mkdir(parents=True, exist_ok=True)
    args.asset.write_text(json.dumps(normalized, indent=2) + "\n", encoding="utf-8")
    statements = [
        "CREATE TABLE IF NOT EXISTS spawn_timer_overrides (",
        "    nickname TEXT PRIMARY KEY COLLATE NOCASE,",
        "    mob_name TEXT NOT NULL COLLATE NOCASE,",
        "    short_name TEXT COLLATE NOCASE,",
        "    zone_name TEXT COLLATE NOCASE,",
        "    continent TEXT,",
        "    respawn_minutes INTEGER NOT NULL CHECK(respawn_minutes >= 0),",
        "    window_minutes INTEGER NOT NULL DEFAULT 0 CHECK(window_minutes >= 0),",
        "    windows_json TEXT,",
        "    last_known_tod TEXT,",
        "    source TEXT NOT NULL DEFAULT 'manual',",
        "    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP",
        ");",
        "",
        "CREATE INDEX IF NOT EXISTS idx_spawn_timer_override_mob",
        "ON spawn_timer_overrides(mob_name COLLATE NOCASE);",
        "",
    ]
    for nickname, row in normalized.items():
        statements.append(
            "INSERT OR IGNORE INTO spawn_timer_overrides(" 
            "nickname,mob_name,short_name,zone_name,continent,respawn_minutes," 
            "window_minutes,windows_json,last_known_tod,source) VALUES(" 
            f"{sql(nickname)},{sql(row['name'])},{sql(row['shortName'])},{sql(row['zone'])}," 
            f"{sql(row['continent'])},{row['respawnMinutes']},{row['windowMinutes']}," 
            f"{sql(row['windows'])},{sql(row['lastKnownTod'])},'bundled-json');"
        )
    statements.extend([
        "",
        "INSERT OR IGNORE INTO schema_migrations(version, name)",
        "VALUES (43, 'spawn timer override catalog');",
        "",
    ])
    args.migration.write_text("\n".join(statements), encoding="utf-8")
    print(f"Imported {len(normalized)} spawn overrides")


if __name__ == "__main__":
    main()
