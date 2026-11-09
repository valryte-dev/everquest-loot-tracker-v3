# Portrait Studio

Portrait Studio turns the existing EverQuest model renderer into a reusable composition and illustration workflow without changing character, inventory, or model-pack data.

## Workflow

1. Open **Roster > Portrait Studio**.
2. Add saved characters. Race, gender, armor, tints, weapons, shields, and item effects come from the same profiles and inventory snapshots used by Character Details.
3. Choose a formation, drag the name handle above a model, and fine-tune its size and position.
4. Use **Export exact PNG** to save the unmodified model lineup.
5. Optionally configure an AI provider and send that exact PNG as the visual reference for an illustrated version.

Projects are portable versioned JSON files stored under the application-data `portrait-projects` directory. They contain names and layout coordinates only; they do not duplicate inventory records or API credentials.

## Providers and credentials

API keys are stored in the operating-system credential vault. They are never written to SQLite, diagnostic logs, portrait project files, or exported PNGs.

| Provider | Capability |
| --- | --- |
| OpenAI / ChatGPT | Generate or edit an image using the exact model lineup as a reference. |
| Google Gemini | Generate or edit an image using the exact model lineup as a reference. |
| Anthropic Claude | Inspect the reference and refine the art prompt. Claude does not return a generated image. |
| OpenAI-compatible | Use a compatible Images API over HTTPS, or a local endpoint on `localhost`. |
| Cursor | Informational entry only. Cursor is an editor that connects to model providers, not an image-generation API. |

Provider model names remain editable so users can adopt newer or provider-specific models without an application release.

## Safety and performance

- The normal character viewer retains its fast render settings. Frame-buffer preservation is enabled only inside Portrait Studio, where PNG capture needs it.
- Provider calls happen only after an explicit button press and never run in the log watcher or combat parser.
- Reference images are limited to 16 MB and prompts to 12,000 characters.
- Custom endpoints require HTTPS, except for `localhost` development servers.
- Deleting a project removes only its JSON layout file. It does not affect characters, inventory, model packs, or generated images already downloaded by the user.

## Rollback

The pre-feature checkpoint is `pre-portrait-studio-2026-11-09`. Portrait Studio adds no database migration. Rolling back leaves its project JSON files and operating-system vault entries unused; they can be removed independently later.
