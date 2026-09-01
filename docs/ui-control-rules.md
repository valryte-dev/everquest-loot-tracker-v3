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
