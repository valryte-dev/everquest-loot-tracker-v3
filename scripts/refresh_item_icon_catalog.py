"""Refresh the bundled P99 item-name/PEQ-ID to icon catalog.

P99 Planner publishes a compact SQLite item catalog and deduplicated icon atlas.
This build-time utility converts that public catalog into an app-owned TSV and
downloads each distinct icon once. The desktop app never needs the network to
render inventory or equipment icons.
"""

from __future__ import annotations

import concurrent.futures
import sqlite3
import tempfile
import urllib.request
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
DATABASE_URL = "https://p99planner.com/data/p99.sqlite"
ICON_URL = "https://p99planner.com/icons/Item_{icon_id}.png"
CATALOG_PATH = ROOT / "src-tauri" / "assets" / "p99-item-icons.tsv"
ICON_DIRECTORY = ROOT / "public" / "item-icons"


def download(url: str, destination: Path) -> None:
    request = urllib.request.Request(url, headers={"User-Agent": "EverQuest-Loot-Tracker/3"})
    with urllib.request.urlopen(request, timeout=30) as response:
        destination.write_bytes(response.read())


def main() -> None:
    ICON_DIRECTORY.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as temporary:
        database_path = Path(temporary) / "p99.sqlite"
        download(DATABASE_URL, database_path)
        connection = sqlite3.connect(database_path)
        rows = connection.execute(
            """SELECT id, COALESCE(peqId, 0), name, icon
               FROM items
               WHERE icon IS NOT NULL AND icon > 0
               ORDER BY id"""
        ).fetchall()
        connection.close()

    CATALOG_PATH.parent.mkdir(parents=True, exist_ok=True)
    with CATALOG_PATH.open("w", encoding="utf-8", newline="\n") as catalog:
        catalog.write("planner_item_id\tpeq_item_id\titem_name\ticon_id\n")
        for planner_id, peq_id, name, icon_id in rows:
            clean_name = str(name).replace("\t", " ").replace("\r", " ").replace("\n", " ")
            catalog.write(f"{planner_id}\t{peq_id}\t{clean_name}\t{icon_id}\n")

    icon_ids = sorted({int(row[3]) for row in rows})

    def fetch(icon_id: int) -> str:
        destination = ICON_DIRECTORY / f"Item_{icon_id}.png"
        if not destination.exists():
            download(ICON_URL.format(icon_id=icon_id), destination)
        return destination.name

    with concurrent.futures.ThreadPoolExecutor(max_workers=12) as pool:
        list(pool.map(fetch, icon_ids))

    print(f"Wrote {len(rows):,} item associations and {len(icon_ids):,} icons.")


if __name__ == "__main__":
    main()
