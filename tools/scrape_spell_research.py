#!/usr/bin/env python3
"""Build the bundled Project 1999 spell-research recipe catalog.

Runtime remains offline. Run this maintainer tool deliberately, review the TSV
and tests, then ship the generated snapshot with the app.
"""
from __future__ import annotations

import csv
import hashlib
import html
import json
import re
import ssl
import tempfile
import time
import urllib.parse
import urllib.request
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from pathlib import Path

API = "https://wiki.project1999.com/api.php"
SOURCE_PAGE = "Skill Research"
SOURCE_URL = "https://wiki.project1999.com/Research"
ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "src-tauri" / "assets" / "p99-spell-research.tsv"
CLASSES = ("Enchanter", "Magician", "Necromancer", "Wizard")
LINK = re.compile(r"\[\[([^\]|#]+)(?:\|([^\]]+))?\]\]")
ROW = re.compile(r"<tr\b[^>]*>(.*?)</tr>", re.I | re.S)
CELL = re.compile(r"<t[dh]\b[^>]*>(.*?)</t[dh]>", re.I | re.S)
ITEM_PAGE = re.compile(r"\{\{\s*Itempage\b", re.I)
SPELL_PAGE = re.compile(r"\{\{\s*Spellpage", re.I)
ICON = re.compile(r"\|\s*lucy_img_ID\s*=\s*(\d+)", re.I)
ITEM_NAME = re.compile(r"\|\s*itemname\s*=\s*([^\r\n]+)", re.I)
TAG = re.compile(r"<[^>]+>")
TEMPLATE = re.compile(r"\{\{[^{}]*\}\}")

@dataclass(frozen=True)
class Component:
    name: str
    icon_id: int | None
    kind: str
    quantity: int = 1

@dataclass(frozen=True)
class Recipe:
    class_name: str
    level: int
    spell_name: str
    trivial: str
    research_only: bool
    availability: str
    components: tuple[Component, ...]

class Wiki:
    def __init__(self) -> None:
        self.context = ssl._create_unverified_context()
        self.cache_dir = Path(tempfile.gettempdir()) / "eq-loot-tracker-p99-cache"
        self.cache_dir.mkdir(exist_ok=True)

    def source(self, title: str) -> str:
        cache = self.cache_dir / f"{hashlib.sha256(title.encode()).hexdigest()}.txt"
        if cache.exists():
            return cache.read_text(encoding="utf-8")
        query = urllib.parse.urlencode({"action":"parse","page":title,"prop":"wikitext","format":"json"})
        request = urllib.request.Request(f"{API}?{query}", headers={"User-Agent":"EverQuestLootTrackerCatalog/3"})
        with urllib.request.urlopen(request, context=self.context, timeout=30) as response:
            payload = json.load(response)
        if "error" in payload:
            raise RuntimeError(payload["error"].get("info", title))
        value = payload["parse"]["wikitext"]["*"]
        cache.write_text(value, encoding="utf-8")
        time.sleep(.05)
        return value


def clean(value: str) -> str:
    value = TAG.sub(" ", value)
    value = TEMPLATE.sub(" ", value)
    value = LINK.sub(lambda m: m.group(2) or m.group(1), value)
    return re.sub(r"\s+", " ", html.unescape(value)).strip()


def links(value: str) -> list[tuple[str,str]]:
    return [(m.group(1).strip().replace("_"," "), (m.group(2) or m.group(1)).strip().replace("_"," ")) for m in LINK.finditer(value)]


def section(source: str, class_name: str) -> str:
    heading = f"=== {class_name} Spell Recipes ==="
    start = source.index(heading) + len(heading)
    match = re.search(r"^={2,3}[^=].*?={2,3}\s*$", source[start:], re.M)
    return source[start:start + match.start()] if match else source[start:]


def kind_for(name: str) -> str:
    folded = name.casefold()
    if folded.startswith("words ") or folded.startswith("word "):
        return "word"
    if folded.startswith("rune "):
        return "rune"
    if any(token in folded for token in ("page", "pg.", "writ", "grimoire", "tome")):
        return "page"
    if folded.startswith("spell: "):
        return "spell"
    return "other"


def parse_rows(source: str) -> list[tuple[str,int,str,str,bool,str,list[tuple[str,str]]]]:
    parsed=[]
    for class_name in CLASSES:
        for row in ROW.findall(section(source,class_name)):
            cells=CELL.findall(row)
            if len(cells)<5 or not clean(cells[0]).isdigit():
                continue
            spell_links=links(cells[1])
            if not spell_links:
                continue
            spell_name=spell_links[0][1]
            availability=clean(cells[3]) or "Unknown"
            lowered=availability.casefold()
            research_only=("false" in lowered or lowered=="no") and "*" not in availability
            components=[]
            for cell in cells[4:]:
                cell_links=links(cell)
                if cell_links:
                    components.append(cell_links[0])
            if components:
                parsed.append((class_name,int(clean(cells[0])),spell_name,clean(cells[2]),research_only,availability,components))
    return parsed


def canonical_component(wiki: Wiki, target: str, display: str, spell_names: set[str]) -> Component:
    if display.casefold() in spell_names or target.casefold() in spell_names:
        name=f"Spell: {display}"
        return Component(name,None,"spell")
    try:
        source=wiki.source(target)
    except Exception:
        return Component(display,None,kind_for(display))
    if SPELL_PAGE.search(source):
        return Component(f"Spell: {display}",None,"spell")
    if not ITEM_PAGE.search(source):
        return Component(display,None,kind_for(display))
    item=ITEM_NAME.search(source)
    icon=ICON.search(source)
    name=clean(item.group(1)) if item else display
    return Component(name,int(icon.group(1)) if icon else None,kind_for(name))


def build() -> list[Recipe]:
    wiki=Wiki()
    raw=parse_rows(wiki.source(SOURCE_PAGE))
    spell_names={row[2].casefold() for row in raw}
    unique={(target,display) for *_,components in raw for target,display in components}
    resolved={}
    with ThreadPoolExecutor(max_workers=8) as pool:
        futures={pair:pool.submit(canonical_component,wiki,*pair,spell_names) for pair in unique}
        for pair,future in futures.items():
            resolved[pair]=future.result()
    recipes=[]
    for class_name,level,spell,trivial,research_only,availability,components in raw:
        expanded=[resolved[pair] for pair in components]
        counts=Counter(component.name for component in expanded)
        by_name={component.name:component for component in expanded}
        normalized=tuple(Component(name,by_name[name].icon_id,by_name[name].kind,counts[name]) for name in counts)
        recipes.append(Recipe(class_name,level,spell,trivial,research_only,availability,normalized))
    return recipes


def main() -> None:
    recipes=build()
    with OUTPUT.open("w",encoding="utf-8",newline="") as handle:
        writer=csv.writer(handle,delimiter="\t",lineterminator="\n")
        writer.writerow(("class_name","level","spell_name","trivial","research_only","availability","source_url","component_name","icon_id","quantity","component_kind"))
        for recipe in recipes:
            for component in recipe.components:
                writer.writerow((recipe.class_name,recipe.level,recipe.spell_name,recipe.trivial,int(recipe.research_only),recipe.availability,SOURCE_URL,component.name,component.icon_id or "",component.quantity,component.kind))
    print(f"wrote {len(recipes)} recipes / {sum(len(r.components) for r in recipes)} component rows to {OUTPUT}")
    for class_name in CLASSES:
        rows=[r for r in recipes if r.class_name==class_name]
        print(f"{class_name}: {len(rows)} recipes, {len({c.name for r in rows for c in r.components})} component types")

if __name__ == "__main__":
    main()