# Character model packs

Character models are optional external assets. They are not stored in SQLite and are not embedded in application releases.

## User flow

The System page contains the **Character model assets** card.

- **Download appearance pack** downloads the 26 classic player race/gender bodies, their default heads, the 12 supported robe-body variants, referenced textures, and supplemental source particle sprites omitted from converted GLB metadata from P99 Planner.
- **Use an external pack folder** activates an existing compatible folder through the native Browse control.
- **Verify files** recomputes every SHA-256 checksum.
- **Disconnect** stops using the pack but deliberately preserves its files.

The Characters page prefers a verified local pack. If a local model cannot be loaded, the viewer retries the P99 Planner online source. If both sources fail, the equipment and inventory interface remains usable and the viewer shows a retry state.

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
            +-- textures/

External folders are used in place. Disconnecting never deletes an external folder.

Equipped armor and weapon appearances are resolved from the embedded P99 item metadata catalog. Gear models and alternate armor textures use the online source when they are not present in a connected external pack; failure to load one visual never hides its inventory or equipment record.

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

## Atomic download behavior

Downloads are written into a staging directory. The complete staging pack is checksum-verified before it can replace the active downloaded pack. If activation fails, the previous pack is restored. Progress is published through the global system-task banner and network work runs outside the UI thread.

## Content ownership

The application downloads source assets on the user's machine and records their source URL. It does not redistribute those assets inside the executable or repository. Downloaded content remains subject to the source's terms.
