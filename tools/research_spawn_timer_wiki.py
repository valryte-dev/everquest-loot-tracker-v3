#!/usr/bin/env python3
"""Collect reviewable Project 1999 wiki spawn-timer evidence.

This is deliberately a research/report tool. It never mutates the managed timer
catalog because wiki prose is not consistent enough for unattended writes.
"""

from __future__ import annotations

import argparse
import json
import re
import urllib.parse
import urllib.request
from pathlib import Path


API = "https://wiki.project1999.com/api.php"
EVIDENCE = re.compile(r"\b(respawn|spawn time|variance|window|skip)\b", re.I)
INFOBOX = re.compile(
    r"^\|\s*(respawntime|respawn(?:\s+time)?|spawn(?:\s+time)?)\s*=\s*(.+?)\s*$",
    re.I | re.M,
)


def clean(value: str) -> str:
    value = re.sub(r"<!--.*?-->", "", value)
    value = re.sub(r"<ref[^>]*>.*?</ref>|<ref[^>]*/>", "", value, flags=re.I)
    value = re.sub(r"\[\[(?:[^]|]+\|)?([^]]+)\]\]", r"\1", value)
    value = re.sub(r"\{\{[^{}]*\}\}", "", value)
    value = re.sub(r"'{2,}", "", value)
    return re.sub(r"\s+", " ", value).strip(" |")


def fetch(titles: list[str]) -> list[dict[str, object]]:
    query = urllib.parse.urlencode(
        {
            "action": "query",
            "prop": "revisions",
            "rvprop": "content",
            "rvslots": "main",
            "format": "json",
            "formatversion": "2",
            "redirects": "1",
            "titles": "|".join(titles),
        }
    )
    request = urllib.request.Request(f"{API}?{query}", headers={"User-Agent": "EQ Loot Tracker spawn timer research"})
    with urllib.request.urlopen(request, timeout=30) as response:
        return json.load(response)["query"]["pages"]


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("catalog", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()

    catalog = json.loads(args.catalog.read_text(encoding="utf-8"))
    requested = {row["name"]: nickname for nickname, row in catalog.items()}
    requested.setdefault("Stormfeather", "stormfeather")
    rows: list[dict[str, object]] = []
    names = list(requested)
    for start in range(0, len(names), 20):
        for page in fetch(names[start : start + 20]):
            title = page.get("title", "")
            revision = (page.get("revisions") or [{}])[0]
            text = revision.get("slots", {}).get("main", {}).get("content", "")
            infobox = [clean(match.group(2)) for match in INFOBOX.finditer(text)]
            evidence = []
            for line in text.splitlines():
                candidate = clean(line)
                if candidate and EVIDENCE.search(candidate) and candidate not in evidence:
                    evidence.append(candidate[:500])
            nickname = requested.get(title)
            if nickname is None:
                lowered = title.casefold()
                nickname = next((nick for name, nick in requested.items() if name.casefold() == lowered), "")
            rows.append(
                {
                    "nickname": nickname,
                    "requestedName": next((name for name, nick in requested.items() if nick == nickname), title),
                    "wikiTitle": title,
                    "wikiUrl": "https://wiki.project1999.com/" + urllib.parse.quote(title.replace(" ", "_")),
                    "missing": bool(page.get("missing")),
                    "infoboxRespawn": infobox,
                    "evidence": evidence,
                }
            )
    rows.sort(key=lambda row: str(row["requestedName"]).casefold())
    args.output.write_text(json.dumps(rows, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"Wrote {len(rows)} reviewed pages to {args.output}")


if __name__ == "__main__":
    main()
