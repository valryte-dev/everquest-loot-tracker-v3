#!/usr/bin/env python3
"""Build the bundled P99 quest-item catalog.

The runtime never depends on the wiki. Run this maintainer tool deliberately,
review the diff, and ship the generated TSV with the application.
"""

from __future__ import annotations

import json
import hashlib
import re
import ssl
import sys
import tempfile
import time
import urllib.parse
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from pathlib import Path

API = "https://wiki.project1999.com/api.php"
ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "src-tauri" / "assets" / "p99-quest-items.tsv"
ICON_OUTPUT = ROOT / "public" / "quest-items"
CLASSES = (
    "Bard", "Cleric", "Druid", "Enchanter", "Magician", "Monk", "Necromancer",
    "Paladin", "Ranger", "Rogue", "Shadow Knight", "Shaman", "Warrior", "Wizard",
)

LINK = re.compile(r"\[\[([^\]|#]+)(?:\|[^\]]+)?\]\]")
TRANSCLUSION = re.compile(r"\{\{:\s*([^}|]+)(?:\|[^}]*)?\}\}")
HEADING = re.compile(r"^==\s*([^=]+?)\s*==\s*$", re.MULTILINE)
ITEM_PAGE = re.compile(r"\{\{\s*Itempage\b", re.IGNORECASE)
ICON = re.compile(r"\|\s*lucy_img_ID\s*=\s*(\d+)", re.IGNORECASE)
BAD_NAMESPACES = ("Category:", "File:", "Image:", "Special:", "Template:", "User:")
REVIEWED_UNPAGED_ITEMS = {"Token of Mastery"}
EPIC_REWARDS = {
    "Bard": ("Singing Short Sword",),
    "Cleric": ("Water Sprinkler of Nem Ankh",),
    "Druid": ("Nature Walkers Scimitar",),
    "Enchanter": ("Staff of the Serpent",),
    "Magician": ("Orb of Mastery",),
    "Monk": ("Celestial Fists",),
    "Necromancer": ("Scythe of the Shadowed Soul",),
    "Paladin": ("Fiery Defender",),
    "Ranger": ("Swiftwind", "Earthcaller"),
    "Rogue": ("Ragebringer",),
    "Shadow Knight": ("Innoruuk's Curse",),
    "Shaman": ("Spear of Fate",),
    "Warrior": ("Jagged Blade of War", "Blade of Strategy", "Blade of Tactics"),
    "Wizard": ("Staff of the Four",),
}


@dataclass(frozen=True)
class Item:
    name: str
    icon_id: int | None


@dataclass(frozen=True)
class Row:
    category: str
    class_name: str
    quest_name: str
    reward: Item
    component: Item
    slot: str = ""
    faction: str = ""
    note: str = ""
    source_url: str = ""


class Wiki:
    def __init__(self) -> None:
        self.context = ssl._create_unverified_context()
        self.cache: dict[str, str] = {}
        self.cache_dir = Path(tempfile.gettempdir()) / "eq-loot-tracker-p99-cache"
        self.cache_dir.mkdir(exist_ok=True)

    def source(self, title: str) -> str:
        if title in self.cache:
            return self.cache[title]
        cache_path = self.cache_dir / f"{hashlib.sha256(title.encode('utf-8')).hexdigest()}.txt"
        if cache_path.exists():
            value = cache_path.read_text(encoding="utf-8")
            self.cache[title] = value
            return value
        query = urllib.parse.urlencode({"action": "parse", "page": title, "prop": "wikitext", "format": "json"})
        request = urllib.request.Request(f"{API}?{query}", headers={"User-Agent": "EverQuestLootTrackerCatalog/3"})
        with urllib.request.urlopen(request, context=self.context, timeout=30) as response:
            payload = json.load(response)
        if "error" in payload:
            raise RuntimeError(f"{title}: {payload['error'].get('info', 'wiki error')}")
        value = payload["parse"]["wikitext"]["*"]
        cache_path.write_text(value, encoding="utf-8")
        self.cache[title] = value
        time.sleep(0.08)
        return value

    def item(self, title: str) -> Item | None:
        title = clean_title(title)
        if not title or title.startswith(BAD_NAMESPACES):
            return None
        source = None
        last_error: Exception | None = None
        # Older P99 item titles inconsistently use ASCII apostrophes and
        # backticks. MediaWiki does not normalize those characters, so probe
        # both spellings but keep one canonical name in the bundled catalog.
        for wiki_title in wiki_title_variants(title):
            try:
                source = self.source(wiki_title)
                break
            except Exception as error:
                last_error = error
        if source is None:
            if title in REVIEWED_UNPAGED_ITEMS:
                return Item(title, None)
            print(f"skip {title}: {last_error}", file=sys.stderr)
            return None
        if not ITEM_PAGE.search(source):
            return None
        item_name = re.search(r"\|\s*itemname\s*=\s*([^\r\n]+)", source, re.IGNORECASE)
        icon = ICON.search(source)
        return Item(clean_title(item_name.group(1) if item_name else title), int(icon.group(1)) if icon else None)


def clean_title(value: str) -> str:
    return re.sub(r"\s+", " ", value.replace("_", " ").strip()).replace("`", "'")


def wiki_title_variants(title: str) -> list[str]:
    """Return every apostrophe/backtick spelling used by legacy wiki pages."""
    positions = [index for index, char in enumerate(title) if char == "'"]
    variants: list[str] = []
    for mask in range(1 << len(positions)):
        chars = list(title)
        for bit, index in enumerate(positions):
            if mask & (1 << bit):
                chars[index] = "`"
        variants.append("".join(chars))
    return variants


def page_url(title: str) -> str:
    return "https://wiki.project1999.com/" + urllib.parse.quote(title.replace(" ", "_"), safe="_()/:'")


def candidates(text: str) -> list[str]:
    seen: set[str] = set()
    result: list[str] = []
    for match in (*TRANSCLUSION.findall(text), *LINK.findall(text)):
        title = clean_title(match)
        if title and title.casefold() not in seen:
            seen.add(title.casefold())
            result.append(title)
    return result


def section(text: str, name: str) -> str:
    match = re.search(rf"^==\s*{re.escape(name)}\s*==\s*$", text, re.MULTILINE | re.IGNORECASE)
    if not match:
        return ""
    end = HEADING.search(text, match.end())
    return text[match.end(): end.start() if end else len(text)]


def sky_rows(wiki: Wiki) -> list[Row]:
    rows: list[Row] = []
    for class_name in CLASSES:
        title = f"{class_name} Plane of Sky Tests"
        try:
            text = wiki.source(title)
        except Exception:
            title = f"{class_name} Plane of Sky Test"
            text = wiki.source(title)
        checklist = section(text, "Checklist")
        starts = [
            match.start()
            for match in re.finditer(
                r"<li>\s*(?:<span[^>]*>)?\s*\{\{:", checklist, flags=re.IGNORECASE
            )
        ]
        for index, start in enumerate(starts):
            block = checklist[start: starts[index + 1] if index + 1 < len(starts) else len(checklist)]
            names = TRANSCLUSION.findall(block)
            if len(names) < 2:
                continue
            reward = wiki.item(names[0])
            if not reward:
                continue
            components = [item for name in names[1:] if (item := wiki.item(name)) and item.name.casefold() != reward.name.casefold()]
            if not components:
                continue
            nearby = checklist[max(0, start - 500):start]
            quest_hints = re.findall(r"(?:Test of|Test Of)\s+([^\r\n<-]+)", nearby)
            quest_name = (
                f"{class_name} Test of {quest_hints[-1].strip()}"
                if quest_hints
                else f"{class_name}: {reward.name}"
            )
            for component in dict.fromkeys(components):
                rows.append(Row("plane_of_sky", class_name, quest_name, reward, component, source_url=page_url(title)))
    return rows


def epic_rows(wiki: Wiki) -> list[Row]:
    rows: list[Row] = []
    index = wiki.source("Class Epic Quest List")
    epic_titles = [clean_title(name) for name in LINK.findall(index) if clean_title(name).endswith("Epic Quest")]
    for title in epic_titles:
        class_name = title.removesuffix(" Epic Quest")
        text = wiki.source(title)
        reward_items = [item for name in EPIC_REWARDS.get(class_name, ()) if (item := wiki.item(name))]
        reward = reward_items[0] if reward_items else next(
            (item for name in candidates(section(text, "Reward")) if (item := wiki.item(name))),
            None,
        )
        if not reward:
            print(f"skip epic without reward: {title}", file=sys.stderr)
            continue
        if len(reward_items) > 1:
            reward = Item(" + ".join(item.name for item in reward_items), reward.icon_id)
        checklist_candidates = candidates(section(text, "Checklist"))
        item_candidates = [name for name in checklist_candidates if wiki.item(name)]
        if not item_candidates:
            item_candidates = candidates(text)
        for name in item_candidates:
            component = wiki.item(name)
            if not component or component.name.casefold() == reward.name.casefold():
                continue
            rows.append(Row("epic", class_name, title, reward, component, source_url=page_url(title)))
    return rows


VELIOUS_CLASS_ARMOR = {
    "Bard": "chain", "Cleric": "plate", "Druid": "leather", "Enchanter": "cloth",
    "Magician": "cloth", "Monk": "leather", "Necromancer": "cloth", "Paladin": "plate",
    "Ranger": "chain", "Rogue": "chain", "Shadow Knight": "plate", "Shaman": "chain",
    "Warrior": "plate", "Wizard": "cloth",
}
VELIOUS_PREFIXES = {
    "Thurgadin": {"plate": "Corroded Plate", "chain": "Corroded Chain", "leather": "Eroded Leather", "cloth": "Torn Enchanted Silk"},
    "Kael": {"plate": "Ancient Tarnished Plate", "chain": "Ancient Tarnished Chain", "leather": "Ancient Leather", "cloth": "Ancient Silk"},
    "Skyshrine": {"plate": "Unadorned Plate", "chain": "Unadorned Chain", "leather": "Unadorned Leather", "cloth": "Tattered Silk"},
}


def velious_component_name(faction: str, armor_type: str, slot: str, suffix: str) -> str:
    # The wiki's plate family labels are regular except for these canonical
    # item-page names. Keep the exceptions explicit and regression-testable.
    if armor_type == "plate" and slot == "Chest":
        return {"Thurgadin": "Corroded Breastplate", "Kael": "Ancient Tarnished Breastplate", "Skyshrine": "Unadorned Breastplate"}[faction]
    if armor_type == "plate" and faction == "Kael" and slot in {"Legs", "Arms"}:
        return f"Ancient Tarnished {suffix}"
    if armor_type == "plate" and faction == "Kael" and slot == "Wrist":
        return "Ancient Tarnished Plate Bracelet"
    return f"{VELIOUS_PREFIXES[faction][armor_type]} {suffix}"
VELIOUS_SUFFIXES = {
    "plate": {"Chest": "Breastplate", "Legs": "Greaves", "Arms": "Vambraces", "Head": "Helmet", "Hands": "Gauntlets", "Feet": "Boots", "Wrist": "Bracer"},
    "chain": {"Chest": "Tunic", "Legs": "Leggings", "Arms": "Sleeves", "Head": "Coif", "Hands": "Gauntlets", "Feet": "Boots", "Wrist": "Bracer"},
    "leather": {"Chest": "Tunic", "Legs": "Leggings", "Arms": "Sleeves", "Head": "Cap", "Hands": "Gloves", "Feet": "Boots", "Wrist": "Bracelet"},
    "cloth": {"Chest": "Robe", "Legs": "Pantaloons", "Arms": "Sleeves", "Head": "Turban", "Hands": "Gloves", "Feet": "Boots", "Wrist": "Wristband"},
}


def velious_rows(wiki: Wiki) -> list[Row]:
    rows: list[Row] = []
    source = page_url("Velious Class Armor")
    for class_name, armor_type in VELIOUS_CLASS_ARMOR.items():
        for faction, prefixes in VELIOUS_PREFIXES.items():
            for slot, suffix in VELIOUS_SUFFIXES[armor_type].items():
                component_name = velious_component_name(faction, armor_type, slot, suffix)
                component = wiki.item(component_name)
                if not component:
                    print(f"missing Velious component: {component_name}", file=sys.stderr)
                    continue
                reward = Item(f"{class_name} {faction} {slot} armor", None)
                rows.append(Row("velious_armor", class_name, f"{class_name} {faction} Armor", reward, component, slot, faction, armor_type, source))
    return rows


def safe(value: object) -> str:
    return str(value if value is not None else "").replace("\t", " ").replace("\r", " ").replace("\n", " ")


def download_icon(icon_id: int, context: ssl.SSLContext) -> None:
    destination = ICON_OUTPUT / f"Item_{icon_id}.png"
    if destination.exists() and destination.stat().st_size > 100:
        return
    request = urllib.request.Request(
        f"https://wiki.project1999.com/images/Item_{icon_id}.png",
        headers={"User-Agent": "EverQuestLootTrackerCatalog/3"},
    )
    with urllib.request.urlopen(request, context=context, timeout=30) as response:
        content = response.read()
    if not content.startswith(b"\x89PNG"):
        raise RuntimeError(f"Item_{icon_id}.png did not return a PNG")
    destination.write_bytes(content)


def main() -> None:
    wiki = Wiki()
    rows = sky_rows(wiki) + velious_rows(wiki) + epic_rows(wiki)
    unique = sorted(set(rows), key=lambda row: (row.category, row.class_name, row.quest_name, row.component.name))
    header = ("category", "class_name", "quest_name", "reward_name", "reward_icon_id", "component_name", "component_icon_id", "quantity", "slot", "faction", "note", "source_url")
    lines = ["\t".join(header)]
    for row in unique:
        lines.append("\t".join(map(safe, (row.category, row.class_name, row.quest_name, row.reward.name, row.reward.icon_id, row.component.name, row.component.icon_id, 1, row.slot, row.faction, row.note, row.source_url))))
    OUTPUT.write_text("\n".join(lines) + "\n", encoding="utf-8")
    ICON_OUTPUT.mkdir(parents=True, exist_ok=True)
    icon_ids = sorted(
        {
            icon_id
            for row in unique
            for icon_id in (row.reward.icon_id, row.component.icon_id)
            if icon_id is not None
        }
    )
    with ThreadPoolExecutor(max_workers=8) as pool:
        list(pool.map(lambda icon_id: download_icon(icon_id, wiki.context), icon_ids))
    counts = {category: sum(row.category == category for row in unique) for category in ("plane_of_sky", "velious_armor", "epic")}
    print(f"wrote {OUTPUT}: {len(unique)} rows {counts}; {len(icon_ids)} icons")


if __name__ == "__main__":
    main()
