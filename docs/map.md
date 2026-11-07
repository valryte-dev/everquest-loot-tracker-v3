# EverQuest Map

The Map workspace renders the local EqTool map pack and follows the active character from normal EverQuest log messages.

## How tracking works

- Zone selection follows `You have entered ...` and zone `/who` evidence already used elsewhere in the app.
- Position follows `Your Location is X, Y, Z`, produced by EverQuest's `/loc` command.
- The app does not send keystrokes or automate the game. Put `/loc` on a hotkey or macro when frequent updates are wanted.
- The latest zone and position are retained per character, so reopening the Map page does not require rescanning logs.
- Movement between consecutive `/loc` readings supplies the marker heading.

## Map controls

- Drag to pan and use the mouse wheel to zoom. Wheel input is contained by the map and does not scroll the page.
- `Center me` centers the latest player marker.
- `Fit map` restores the complete-zone view.
- Individual EqTool map layers may be hidden.
- `Fade other levels` uses the player's Z coordinate to reduce visual noise in stacked dungeons.
- A zone can be selected manually when the log has not yet supplied current-zone evidence.

## Data source and performance

The map files and zone aliases come from the locally downloaded [EqTool](https://github.com/smasherprog/EqTool) source under its MIT license. The license is shipped at `public/maps/EQTOOL-LICENSE.txt`. Run `scripts/generate-eqtool-map-assets.ps1` after refreshing the reference checkout to regenerate the local pack.

Map geometry is fetched only when this page is open and is drawn on one canvas. It is not included in broad application snapshots and does not add DOM nodes per map line.
