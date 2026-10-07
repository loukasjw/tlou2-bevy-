# Reverse-engineering package for the Bevy prototype

Start with this file, then `VALUES.md` (every usable number in one place). Everything is tagged **confirmed** (read directly from game data or code) or **guess**. "Not found" means I looked and there is nothing to fill in: measure in-game instead, and do not invent values.

## Contents
| File | What it is |
|---|---|
| `VALUES.md` | All numbers found so far, one table per topic, with confidence and source |
| `data/weapon-damages.csv` | Decoded damage table: weapon x target, 240 rows (int + two floats) |
| `notes/HANDOFF.md` | First handoff (background, confidence rules, mapping onto your state machine) |
| `notes/answer.md` | Reply to `question.md` (movement; mostly "not found") |
| `notes/answer2.md` | Reply round 2 and 3 (traversal settings, landing thresholds, jump findings) |
| `notes/answer3.md` | Reply to `question2.md` (weapons, melee, camera, landing direction) |
| `notes/question.md`, `notes/question2.md` | The questions you asked, for reference |
| `tools/` | Optional: archive extractor and data-format helpers (need the game installed) |

## Deliberately NOT included
Raw game data files, decompiled game code, models, audio and textures. They belong to Naughty Dog / Sony. Keep them out of the repo and out of anything you share. The numbers and notes here are findings only; use placeholder meshes.

## Key takeaways
1. Internal names: hunting rifle `rifle-remington-bolt`; pistol `pistol-beretta`; the silencer is a **durable upgrade** of the pistol (it has a health value), not a separate gun.
2. Jumps are **target-based** (the game solves a velocity to reach a chosen edge/point), not a fixed take-off speed and gravity.
3. Weapon handling values are overridden by upgrades by index; the data file only holds base values.
4. Not found: ground speeds, gravity, capsule, stance times, fire rate, reload, magazine, recoil, spread, noise, melee timings, camera offsets. Measure these in-game and tag them `placeholder (measure)`.
