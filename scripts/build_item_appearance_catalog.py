#!/usr/bin/env python3
"""Build the embedded item appearance lookup from P99 Planner's public item DB."""
import argparse
import csv
import sqlite3
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("database", type=Path)
parser.add_argument("output", type=Path)
args = parser.parse_args()
connection = sqlite3.connect(args.database)
rows = list(connection.execute("""
    SELECT id, peqId, name, COALESCE(material, 0), COALESCE(idfile, ''), COALESCE(color, 0), itemType
    FROM items
    WHERE COALESCE(peqId, 0) > 0
      AND (COALESCE(material, 0) > 0 OR LENGTH(TRIM(COALESCE(idfile, ''))) > 0 OR COALESCE(color, 0) > 0)
    ORDER BY peqId, id
"""))
shield_models = {f"IT{value}" for value in (*range(200, 224), 226, 228)}
rows = [
    (*row[:6], 8 if str(row[4]).strip().upper() in shield_models else row[6])
    for row in rows
]
args.output.parent.mkdir(parents=True, exist_ok=True)
with args.output.open("w", encoding="utf-8", newline="") as stream:
    writer = csv.writer(stream, delimiter="\t", lineterminator="\n")
    writer.writerow(("planner_item_id", "peq_item_id", "item_name", "material", "idfile", "color", "item_type"))
    writer.writerows(rows)
