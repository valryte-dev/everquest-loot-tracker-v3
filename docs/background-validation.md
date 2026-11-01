# Background validation

Local verification must not interfere with normal use of the workstation. Launch it with:

```powershell
.\scripts\start-background-validation.cmd -Mode Full
```

The command returns immediately with a process ID plus temporary stdout, stderr, and result-file paths. Check the result JSON or tail the logs to monitor progress.

## Modes

- `Frontend`: frontend tests followed by the TypeScript/Vite production build.
- `Full`: frontend validation, Rust formatting, Rust tests, and strict Clippy.
- `Standalone`: the Tauri standalone executable build only.
- `All`: the full validation suite followed by the standalone build.
- `Smoke`: verifies the background worker limits without compiling or testing.

A focused frontend test can be passed with `-TestPath`, for example:

```powershell
.\scripts\start-background-validation.cmd -Mode Frontend -TestPath src/app/characters/spellCollectionModel.test.ts
```

## Resource controls

The worker applies all controls before it creates compiler or test children:

- hidden, non-interactive child process;
- normal Windows priority inherited by child processes;
- no processor-affinity restriction;
- Cargo, Rayon, Go/esbuild, and Vitest limited to two workers or threads;
- steps run sequentially rather than competing with each other;
- a reusable, ignored `target-background-validation` directory prevents repeated full Rust dependency builds;
- output and completion state are written beneath the system temporary directory.

The default is intentionally conservative. Do not raise `-Jobs` above `2` during normal agent work. A higher limit requires explicit user authorization.

For the opt-in network audit that downloads the live P99 spell category and verifies class-page eligibility:

```powershell
.\scripts\start-background-validation.cmd -Mode SpellCatalogLive
```