# Character model packs

Character models are optional external assets. They are not stored in SQLite and are not embedded in application releases.

## User flow

The System page contains the **Character model assets** card.

- **Download complete model pack** downloads the 26 classic player race/gender bodies, every available alternate head, the 12 supported robe-body variants, every equipment model referenced by the embedded item catalog, every generated armor material/region texture, animated material frames, and all renderer particle sprites from P99 Planner.
- **Use an external pack folder** activates an existing compatible folder through the native Browse control.
- **Verify files** recomputes every SHA-256 checksum.
- **Disconnect** stops using the pack but deliberately preserves its files.

The Characters page prefers a verified local pack and retries the P99 Planner online source when an individual asset is absent. If all configured sources fail, the equipment and inventory interface remains usable and the viewer shows a retry state.

## Storage

Downloaded packs use the platform application-data directory:

    EverQuestLootTracker/
    +-- model-packs/
        +-- p99-classic-appearance-v2/
            +-- model-pack.json
            +-- hum.glb
            +-- hum01.glb
            +-- humhe00.glb
            +-- huf.glb
            +-- items/
                +-- it150.glb
            +-- textures/

External folders are used in place. Disconnecting never deletes an external folder.

Equipped armor and weapon appearances are resolved from the embedded P99 item metadata catalog. Local assets are tried first, with the online source retained as recovery for a missing or damaged individual file; failure to load one visual never hides its inventory or equipment record.

## Manifest format

model-pack.json is the trust boundary for local assets:

    {
      "formatVersion": 1,
      "name": "Example model pack",
      "version": "1",
      "createdAt": "2026-09-16T00:00:00Z",
      "sourceUrl": "https://example.invalid/models/",
      "models": ["hum", "huf"],
      "files": [
        {
          "path": "hum.glb",
          "bytes": 123456,
          "sha256": "lowercase hex SHA-256"
        }
      ]
    }

Every path must be relative and traversal components are rejected. The loopback asset route serves only files explicitly listed in the active manifest.

The loopback server selects the first available port in the app's reserved range and publishes that URL to the viewer. It does not assume port 8765 is free, so the V3 asset pack continues to work alongside a legacy tracker or another local service. Asset requests are served concurrently, and HEAD availability probes do not read full files from disk.

## Atomic download behavior

Downloads are written into a staging directory. The complete staging pack is checksum-verified before it can replace the active downloaded pack. If activation fails, the previous pack is restored. Progress is published through the global system-task banner and network work runs outside the UI thread.

## Content ownership

The application downloads source assets on the user's machine and records their source URL. It does not redistribute those assets inside the executable or repository. Downloaded content remains subject to the source's terms.
