# EverQuest Live Loot Parser Integration Specification

This document is an agent-readable specification of the live-loot parsing behavior in EverQuest Loot Tracker V3. It is intended for implementing a compatible live-loot plugin in another system.

## 1. Scope

The live-loot pipeline has four responsibilities:

1. Discover and tail the active Project 1999 Green character log.
2. Resolve `You` to the character named by that log file.
3. Maintain current-group and most-recent-mob state from surrounding log messages.
4. Emit an immutable loot record containing the item, looter, correlated mob, group snapshot, source file, and source byte offset.

Merchant parsing, inventory imports, split accounting, and item valuation are outside this parser specification.

## 2. Active log discovery

### Accepted filename

Match filenames case-insensitively against:

```text
eqlog_<character>_P1999Green.txt
```

Equivalent regular expression:

```regex
^eqlog_(?<character>.+)_P1999Green\.txt$
```

The current implementation specifically checks the `eqlog_` prefix and `_P1999Green.txt` suffix, then treats everything between them as the character name.

Example:

```text
eqlog_Youngman_P1999Green.txt -> active character = Youngman
```

### Selecting the active file

- Poll the configured Logs directory every 750 milliseconds.
- Filter to accepted log filenames.
- Select the file with the newest filesystem modification time.
- Only one file is expected to be active at a time.
- When the selected filename changes to a different character, clear the current group.
- Merely activating a log remembers its character name, but does not add that character to the current group.

### Initial and ongoing offsets

- When a file is first selected, initialize its read offset to its current size. Existing history is intentionally ignored.
- Open the file read-only. Never write to or exclusively lock the EverQuest log.
- Seek to the remembered byte offset and read appended bytes.
- Process only newline-terminated records.
- Accept `LF` and `CRLF`; remove the terminator before parsing.
- If the final record is incomplete, do not process it and do not advance past its first byte. Retry after a later poll.
- If the file becomes smaller than its remembered offset, assume truncation and restart at byte zero.
- Track offsets independently per file.

## 3. Common log envelope

Every parsed event must first match:

```regex
^\[(?<time>[^]]+)]\s*(?<body>.*)$
```

Parse `time` using this exact English format:

```text
%a %b %d %H:%M:%S %Y
```

Example:

```text
Mon Aug 03 07:09:18 2026
```

The timestamp is a local, timezone-free wall-clock value. If the target system requires an instant, attach the machine's configured local timezone explicitly and define how daylight-saving ambiguity is handled.

If either the envelope or timestamp is invalid, ignore the line.

## 4. Loot event parsing

Apply this anchored, case-sensitive expression to `body`:

```regex
^--(?<who>You|[A-Za-z][A-Za-z'_-]*) (?:have|has) looted (?:an? )?(?<item>.+?)\.--$
```

Behavior:

- `who` is either literal `You` or a character name.
- A character name starts with an ASCII letter and may then contain ASCII letters, apostrophes, underscores, or hyphens.
- Both `have looted` and `has looted` are accepted regardless of which name form was used.
- A leading `a ` or `an ` is removed from the captured item name.
- An article is optional.
- The item ends at the final period immediately before `--`.
- Item text is captured non-greedily but may contain spaces and internal punctuation.
- If `who` equals `You` ignoring case, emit the active character from the filename as the looter.
- Otherwise emit `who` unchanged.

Examples:

```text
[Mon Aug 03 07:09:18 2026] --Vinkledoo has looted a Tears of Prexus.--
=> looter: Vinkledoo
=> itemName: Tears of Prexus

[Mon Aug 03 07:09:18 2026] --You have looted a Tears of Prexus.--
active character: Youngman
=> looter: Youngman
=> itemName: Tears of Prexus

[Mon Aug 03 07:09:19 2026] --Vinkledoo has looted Blue Throne.--
=> looter: Vinkledoo
=> itemName: Blue Throne
```

If a line contains ` looted ` case-insensitively but fails this pattern, the current application records a parser warning. A plugin should expose an equivalent diagnostic rather than silently discarding likely format changes.

## 5. Mob correlation

Two kill forms are recognized.

### Local character kill

```regex
^You have slain (?<mob>.+)!$
```

Example:

```text
[Mon May 11 13:35:38 2026] You have slain a mortiferous golem!
```

Resolve the killer to the active character from the filename.

### Named character kill

```regex
^(?<mob>.+) has been slain by (?<killer>[A-Za-z][A-Za-z'_-]*)!$
```

Example:

```text
[Mon May 11 14:19:22 2026] a mortiferous golem has been slain by Skriz!
```

### Correlation algorithm

- Maintain `lastMobByFile[sourceFile]`.
- Either recognized kill form replaces the value for that source file.
- When loot is parsed, copy the current value into the loot record.
- Mob state is file-specific and process-memory only.
- The mob is not consumed or cleared after one loot event, allowing several drops to correlate to one kill.

Important fidelity note: V3 currently has no maximum time gap, line gap, group-membership check, or intervening-combat invalidation. Therefore an old kill can be assigned to later loot. A new plugin may add a configurable confidence window, but should flag that as a deliberate behavior change rather than claiming exact V3 compatibility.

## 6. Group composition state

Names use the same character-name grammar described above.

### Join

```regex
^(?<name>[A-Za-z][A-Za-z'_-]*) has joined the group\.$
```

Add `name` to the current group. Also add the active character from the filename.

### Leave

```regex
^(?<name>[A-Za-z][A-Za-z'_-]*) has left the group\.$
```

Remove `name` from the current group. Do not remove remembered-name history.

### Group speech as presence evidence

```regex
^(?<name>[A-Za-z][A-Za-z'_-]*) tells the group, .+$
```

Add `name` if absent. Also add the active character from the filename. Repeated speech from an existing member is idempotent and must not cause a UI/data refresh unless state actually changes.

### Local player removed

```regex
^You have been removed from the group\.$
```

Clear the entire current group while preserving remembered names.

### Lifecycle rules

- Start every application/plugin session with an empty current group.
- Clear the group when the active character changes, comparing names case-insensitively.
- Store known names case-insensitively for reuse, but retain their display spelling.
- Manual group add/remove operations may update the same state used by parsing.

## 7. Loot-time snapshot

When a loot event is accepted, immediately snapshot the current parser state. Later group or mob changes must not rewrite an existing loot record. V3 performs the record and member inserts sequentially; a new plugin should wrap them in one transaction for stronger atomicity.

Recommended normalized record:

```json
{
  "eventType": "loot",
  "happenedAtLocal": "2026-08-03T07:09:18",
  "itemName": "Tears of Prexus",
  "looterName": "Youngman",
  "mobName": "a mortiferous golem",
  "groupMembers": ["Youngman", "Posed", "Skriz"],
  "sourceFile": "C:\\EverQuest\\Logs\\eqlog_Youngman_P1999Green.txt",
  "sourceOffset": 18442,
  "rawLine": "[Mon Aug 03 07:09:18 2026] --You have looted a Tears of Prexus.--"
}
```

`mobName` may be `null`. `groupMembers` may be empty. The current implementation does not automatically insert the looter into the snapshot; membership comes from current-group state.

## 8. Deduplication and transaction semantics

Use this source identity:

```text
(sourceFile, sourceOffset, rawLine)
```

Enforce it with a unique constraint or idempotency key. Byte offset alone is insufficient across multiple files.

For each accepted loot event, use one transaction:

1. Insert the loot record if its source identity is new.
2. If inserted, copy every current group member into loot-membership storage.
3. Commit.
4. If it was already present, do not create another group snapshot or duplicate notification.

The source offset is the byte position of the first byte of the original log line, before newline removal.

## 9. Reference state machine

```text
on startup:
  currentGroup = empty set
  activeFile = none
  offsetByFile = empty map
  lastMobByFile = empty map

every 750 ms:
  candidate = newest-modified accepted log
  if candidate changed:
    character = parse character from filename
    if previous character exists and differs case-insensitively:
      currentGroup.clear()
    remember character
    offsetByFile.setdefault(candidate, current file size)
    activeFile = candidate

  if activeFile grew:
    read complete appended lines from offsetByFile[activeFile]
    for each line with its starting byte offset:
      event = parse envelope, then body patterns in this order:
        loot
        merchant listing (outside this specification)
        direct tell (outside this specification)
        local kill
        remote kill
        full group clear
        group join
        group leave
        group speech

      if kill:
        lastMobByFile[activeFile] = event.mobName
      if join or speech:
        currentGroup.add(event.character)
        currentGroup.add(active character)
      if leave:
        currentGroup.remove(event.character)
      if full group clear:
        currentGroup.clear()
      if loot and source identity is new:
        persist loot with lastMobByFile.get(activeFile)
        persist a copy of currentGroup

    advance offset only through the last complete line
```

## 10. Minimum compatibility tests

An implementation should test at least these cases:

1. `You` resolves to the filename character.
2. A named looter remains unchanged.
3. Loot works with `a`, `an`, or no article.
4. A local kill supplies the mob for following loot.
5. A named kill supplies the mob for following loot.
6. Multiple loot lines after one kill retain the same mob.
7. Join adds the named character and local character.
8. Group speech adds a missing character idempotently.
9. Leave removes only the named character.
10. Local removal clears the entire group.
11. Character-file switching clears the group.
12. Startup ignores historical content and begins at EOF.
13. File truncation resets the offset safely.
14. An incomplete final line is retried and emitted exactly once after completion.
15. Two new loot lines have distinct byte offsets and both persist.
16. Re-reading the same line does not duplicate the loot event.
17. A likely loot line that fails parsing emits a diagnostic.

## 11. Source-of-truth implementation

This specification reflects these V3 files at release `v3.3.0`:

- `src-tauri/src/domain/log_events.rs` — envelope and event regular expressions.
- `src-tauri/src/application/runtime.rs` — file discovery, tailing, state transitions, persistence, and diagnostics.
- `src-tauri/src/migrations/000_v2_compatibility.sql` — loot and group persistence constraints.

If code and this document ever disagree, treat the code for the deployed version as authoritative and update this specification alongside the parser change.
