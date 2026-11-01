# File and Folder Selection Controls

Any control that accepts a file or folder path must provide a visible **Browse** action that opens the operating system's native picker.

- Use the shared `PathPicker` component from `src/app/ui.tsx`.
- Set `kind="folder"` for directories and `kind="file"` for files.
- File pickers should provide suitable extension filters when the accepted formats are known.
- Manual path entry may remain available, but it must never be the only selection method.
- Drag-and-drop areas must also provide a Browse fallback for accessibility and discoverability.
- Do not use text prompts to request filesystem paths.
- Keep the behavior compatible with Windows, macOS, and Linux.

This is the default rule for all new and updated filesystem-path controls.

# Countable Work Must Show Progress

Any user-visible operation whose work can be counted must show determinate progress while it runs.

- Display completed and total units together with a progress bar and percentage.
- Prefer the most granular meaningful unit available (bytes or records over files when file sizes vary).
- Show the current item being processed when it helps the user understand forward movement.
- Keep the progress display visible through completion so the final count can be verified.
- Use an indeterminate busy state only when the work truly cannot be measured.
- A status message by itself is not sufficient for a countable scan, import, export, rebuild, purge, reconciliation, or batch mutation.

This rule applies to all new operations and whenever an existing countable operation is updated.
