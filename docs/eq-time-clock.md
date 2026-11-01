# Always-visible EverQuest clock

EverQuest advances game time at twenty times real time:

- 1 real minute = 20 EverQuest minutes
- 3 real minutes = 1 EverQuest hour
- 72 real minutes = 1 full EverQuest day

Reference: [Project 1999 Wiki — Time](https://wiki.project1999.com/Time)

## Synchronization

The shared log parser recognizes messages such as:

```text
[Wed Oct 28 08:53:48 2026] Game Time: Tuesday, September 17, 3253 - 6 AM
```

The envelope timestamp is the real-time synchronization point and the `6 AM` value becomes minute 360 of the EverQuest day. The latest typed synchronization is stored in the existing `app_settings` table as `eq_time_sync` and is returned by the lightweight global-status query.

The header clock projects forward locally at one EverQuest minute per three real seconds. It wraps at midnight and changes between day and night presentation at 6 AM and 8 PM. Every later `Game Time:` message replaces the synchronization point, correcting any drift without restarting the app.

Synchronizations are monotonic by their log timestamp, so a delayed write from an older character log cannot move the clock backward.

Before the first recognized time message, the always-visible clock displays `--:--` with a tooltip instructing the user to run `/time` in EverQuest.

## Architecture and rollback

- Parsing extends the canonical typed `LogEvent` pipeline; no second file reader was added.
- Persistence occurs in the watcher transaction that processes the source line.
- The shell continues using its existing bounded global-status poll; the clock adds no history query.
- No schema migration is required. Older builds ignore the `eq_time_sync` setting, so rollback is non-destructive.
