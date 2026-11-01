#!/usr/bin/env python3
"""Convert EqTool's MIT-licensed zone spawn catalog into a reviewed runtime TSV."""

from __future__ import annotations

import argparse
import re
from pathlib import Path


ZONE_START = re.compile(r'ZoneInfoMap\.Add\("([^"]+)",\s*new\s+ZoneInfo\s*\{')
NPC_START = re.compile(r'new\s+NpcSpawnTime\s*\{')
TIME = re.compile(r'(?:new\s+TimeSpan\s*\(([^)]*)\)|TimeSpan\.From(Hours|Minutes)\s*\(([^)]*)\))')


def matching_brace(text: str, opening: int) -> int:
    depth = 0
    quoted = False
    escaped = False
    for index in range(opening, len(text)):
        char = text[index]
        if quoted:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                quoted = False
            continue
        if char == '"':
            quoted = True
        elif char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return index
    raise ValueError(f"Unclosed brace at character {opening}")


def seconds(match: re.Match[str]) -> int:
    if match.group(1) is not None:
        values = [int(value.strip()) for value in match.group(1).split(",") if value.strip()]
        if len(values) == 1:
            return 0  # EqTool only uses TimeSpan(0), meaning zero ticks.
        if len(values) == 3:
            hours, minutes, secs = values
            return hours * 3600 + minutes * 60 + secs
        raise ValueError(f"Unsupported TimeSpan arguments: {match.group(1)}")
    value = float(match.group(3))
    return round(value * (3600 if match.group(2) == "Hours" else 60))


def top_level_default(block: str) -> int:
    depth = 0
    for match in re.finditer(r"[{}]|RespawnTime\s*=", block):
        token = match.group(0)
        if token == "{":
            depth += 1
        elif token == "}":
            depth -= 1
        elif depth == 1:
            duration = TIME.search(block, match.end())
            if duration:
                return seconds(duration)
    raise ValueError("Zone is missing a top-level RespawnTime")


def npc_rules(block: str, property_name: str) -> list[tuple[str, int]]:
    marker = re.search(rf"{property_name}\s*=\s*new\s+List<NpcSpawnTime>\s*(?:\(\))?\s*\{{", block)
    if not marker:
        return []
    opening = block.find("{", marker.start())
    closing = matching_brace(block, opening)
    list_block = block[opening : closing + 1]
    result: list[tuple[str, int]] = []
    for match in NPC_START.finditer(list_block):
        object_open = list_block.find("{", match.start())
        object_close = matching_brace(list_block, object_open)
        item = list_block[object_open : object_close + 1]
        name = re.search(r'Name\s*=\s*"([^"]+)"', item)
        duration = TIME.search(item)
        if name and duration:
            result.append((name.group(1), seconds(duration)))
    return result


def convert(source: Path, output: Path, revision: str) -> tuple[int, int, int]:
    text = source.read_text(encoding="utf-8-sig")
    zones: list[tuple[str, int, list[tuple[str, int]], list[tuple[str, int]]]] = []
    for match in ZONE_START.finditer(text):
        opening = text.find("{", match.start())
        closing = matching_brace(text, opening)
        block = text[opening : closing + 1]
        zones.append((match.group(1), top_level_default(block), npc_rules(block, "NpcSpawnTimes"), npc_rules(block, "NpcContainsSpawnTimes")))

    aliases = [(match.group(1), match.group(2)) for match in re.finditer(r'ZoneNameMapper\.Add\("([^"]+)",\s*"([^"]+)"\)', text)]
    who_aliases = [(match.group(1), match.group(2)) for match in re.finditer(r'ZoneWhoMapper\.Add\("([^"]+)",\s*"([^"]+)"\)', text)]
    direct = {name.casefold(): key for name, key in aliases}
    for alias, target in who_aliases:
        key = direct.get(target.casefold())
        if key:
            aliases.append((alias, key))

    lines = [
        "# EqTool spawn catalog (MIT), generated from EQToolShared/Zones.cs",
        f"# upstream_revision\t{revision}",
        "# record_type\tzone_key\tname_or_alias\trespawn_seconds",
    ]
    for key, default, exact, contains in zones:
        lines.append(f"default\t{key}\t\t{default}")
        lines.extend(f"exact\t{key}\t{name}\t{duration}" for name, duration in exact)
        lines.extend(f"contains\t{key}\t{name}\t{duration}" for name, duration in contains)
    seen: set[tuple[str, str]] = set()
    for alias, key in [(key, key) for key, *_ in zones] + aliases:
        identity = (alias.casefold(), key.casefold())
        if identity not in seen:
            seen.add(identity)
            lines.append(f"alias\t{key}\t{alias}\t")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    return len(zones), sum(len(row[2]) + len(row[3]) for row in zones), len(seen)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("source", type=Path, help="Path to EqToolShared/Zones.cs")
    parser.add_argument("output", type=Path, help="Destination TSV")
    parser.add_argument("--revision", required=True, help="Pinned EqTool Git revision")
    args = parser.parse_args()
    zones, overrides, aliases = convert(args.source, args.output, args.revision)
    print(f"Wrote {zones} zones, {overrides} NPC overrides, and {aliases} aliases to {args.output}")


if __name__ == "__main__":
    main()
