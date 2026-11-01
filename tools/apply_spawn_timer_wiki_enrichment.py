#!/usr/bin/env python3
"""Apply the reviewed P99 wiki timer normalization to the bundled JSON asset."""

from __future__ import annotations

import json
import sys
from pathlib import Path


def main() -> None:
    path = Path(sys.argv[1])
    rows = json.loads(path.read_text(encoding="utf-8"))
    for row in rows.values():
        if row["respawnMinutes"] == 9600:
            row["respawnMinutes"], row["windowMinutes"] = 10080, 480
        elif row["respawnMinutes"] == 3840:
            row["respawnMinutes"], row["windowMinutes"] = 4320, 480
    for nickname, respawn, variance in (
        ("statue", 7200, 480),
        ("vindi", 480, 60),
        ("magi", 4320, 720),
        ("zlandicar", 6480, 480),
        ("lodi", 1080, 60),
    ):
        rows[nickname]["respawnMinutes"] = respawn
        rows[nickname]["windowMinutes"] = variance
    rows["lodi"]["zone"] = "Iceclad Ocean"
    rows["lodi"]["windows"] = {
        "kind": "cumulativeSkip",
        "maxSkips": 2,
        "sourceUrl": "https://wiki.project1999.com/Lodizal",
    }
    rows["stormfeather"] = {
        "name": "Stormfeather",
        "shortName": None,
        "zone": "Iceclad Ocean",
        "continent": None,
        "lastKnownTod": None,
        "respawnMinutes": 1080,
        "windowMinutes": 60,
        "windows": {
            "kind": "cumulativeSkip",
            "maxSkips": 3,
            "sourceUrl": "https://wiki.project1999.com/Stormfeather",
        },
    }
    path.write_text(json.dumps(rows, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
