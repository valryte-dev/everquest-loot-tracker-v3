# Friends Sync

Friends Sync copies one character's EverQuest `[Friends]` INI section to every other discovered character INI.

## Workflow

1. Configure the EverQuest `Logs` folder on the System page.
2. Open **Roster > Friends Sync**.
3. Choose the source character and review the detected friend names.
4. Add names or remove individual friends, then select **Save source list**.
5. Close EverQuest before saving or synchronizing. The client can overwrite INI changes when it exits.
6. Select **Apply to all characters** and confirm the target count.

Character files are discovered in the EverQuest directory beside `Logs`. The source is tracked by exact INI filename, so identical character names on different servers remain unambiguous.

## Safety and file fidelity

- Source edits update only numbered `Friend0` through `Friend99` slots; unused slots remain `*NULL*`.
- `[Ignored]`, Socials, and every other section are never changed by source editing.
- Synchronization replaces only the complete `[Friends]` section.
- Files without a `[Friends]` section receive one.
- All other sections and non-UTF-8/binary bytes are retained unchanged.
- Every edited or synchronized target file is copied before it is changed.
- Backups are stored under `loot-tracker-backups/friends-edit-<timestamp>` or `loot-tracker-backups/friends-sync-<timestamp>` in the EverQuest directory.
- If a target write fails, the affected file is restored from the new backup.
- Synchronization never rewrites the source character.

An empty source Friends list is valid and clears the populated Friends slots after explicit confirmation. `*NULL*` placeholders are not displayed as names.

## Rollback

Close EverQuest, open the backup path shown in the success message, and copy the desired character INI files back into the EverQuest directory. Backups are intentionally retained until the user removes them.
