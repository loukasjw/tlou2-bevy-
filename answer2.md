# ANSWER 2: deeper research (follow-up to answer.md)

New method that worked: **link data symbols to C++ code.** The exe looks up data tables by 64-bit FNV hash immediates, and the data structs in the `00CD` files are memory images (field offset in file = field offset in the C++ struct). I scanned the exe for 33,795 large immediates, built the function table from `.pdata` (169,108 functions), indexed all 7,958 data files, and found **12,287 hashes shared by data and code**. That tells me which function reads which table. Tools: `dc_tools\`.

Confidence tags as before. Units are NOT stated in the files: where I say metres/degrees/seconds it is a **guess** from the field name or value.

## A. CONFIRMED named values (file `dc1/player-traversal-settings.bin`, field names resolved by hash)
Value read directly from the file; unit guessed.

| Field (exact name in data) | Value | Likely unit (guess) |
|---|---|---|
| `jump-backwards-angle-threshold` | 60 | degrees |
| `jump-backwards-seconds-threshold` | 0.5 | seconds |
| `jump-to-wall-angle-threshold` | 40 | degrees |
| `max-dyno-jump-angle-to-wall` | 30.5 | degrees |
| `evade-off-edge-grab-height` | 2 | metres |
| `run-off-edge-grab-height` | 8 | metres |
| `up-back-dyno-min-xz` / `max-xz` | 0.95 / 2.05 | metres |
| `up-back-dyno-min-y` / `max-y` | 0.45 / 1.55 | metres |
| `edge-look-away-spring` | 6 | spring rate |
| `edge-look-left-right-spring` | 9 | spring rate |
| `water-ladder-depth` | 2.7 | metres |
| 6 unnamed floats | 0.5, 0.6, 1, 1.13, 0.5, 2.3 | unknown |
| 5 unnamed small structs | e.g. (50.0, 50.0, 95.0, 105.0, 89.0) | unknown, probably ranges |

These answer part of your Q6 (traversal). They are probe/edge-grab limits for jumping and edge-grab, not the vault/clamber height limits. Those were not found.

## B. CONFIRMED values, MEANING NOT YET MAPPED
Source `dc1/player-settings.bin` (314 named settings, each a struct). I know each struct's name, which function uses it and its raw floats. I do not yet know which float is which field.

| Struct (name in data) | First fields (raw) | Used by |
|---|---|---|
| `*player-jump-settings*` | 0.25, 2.4, 0.25, 1.24, 1.6, 2.4, 1.5, 3, 3, 5, 2.4, 1, 1, 0.3, 0.75, 2.26, 2.7, 0.9, 1.2, 1.2 | FUN_140e21d20 (jump controller setup) |
| `*player-evade-jump-settings*` | same, except fields 8-10 = 1.5, 1.5, 1.5 and field 16 = 1.3 | FUN_140e1e290 |
| `*player-roll-settings*` | 0.3, 4.5, ... | FUN_140b6fcb0 |
| `*player-sprint-roll-settings*` | 1, 4.8, ... | FUN_140b6fcb0 |
| `*player-mp-roll-settings*` / djinn | 1, 5.75 / 1, 8 | FUN_140b6fcb0 |
| `*player-prone-settings*` | ..., 1.75, 0.4, 100, 120, ..., 1.7, 2.4 | FUN_140c5fc50, FUN_140bbbd40 |
| `*player-bipod-settings*` | 0.5, 0.5, 0.7, 1.2, ..., 0.1, 0.36, 70 | PlayerBipodController |
| `*player-zipline-settings*` | 2, 8, 15, 5, ... | FUN_140d95e40 |
| `player-health-settings` (type) | 1, 100, 0.5, 0, 0.33, 0.5, 0.35, 0.5 | Player::Init |

Observations (guess unless stated): in the roll structs the second float rises 4.5 (normal) -> 4.8 (sprint) -> 5.75 (mp); that looks like a roll speed in m/s, but the field is **unverified**. The `100` in the health struct is probably max health (guess).
The jump and evade-jump structs differ in only a few fields, so those fields are the ones that are mode-dependent.

## C. Landing
`dc1/player-jump-settings.bin` (86 bytes) contains one symbol, `*player-land-speed-threshold*`, a struct whose first two floats are **3.1** and **2.0**. It is read by six landing functions at 140e04ae0, 140e04d00, 140e059a0, 140e05b40, 140e069d0, 140e06b90 (next to `KneeLanding::DoLand` / `ChestLanding::DoLand`). Likely speed thresholds (m/s) that choose the landing type (**guess**). `GroundLanding::DoLand` additionally reads the data symbol `fall-recovery` (in `player-upgrade-settings.bin`), so recovery time is data-driven and upgradeable (**confirmed link**).
`death-height` exists but lives inside a compiled animation-overlay **script** (`player-overlays.bin`, read by `BasicJump::ApplyCorrectJumpType`), so it is script code, not a plain number. Not extracted.

## D. Other small confirmed values
- `joypad.bin`: `*joypad-shake-threshold*` = 1, `*joypad-stick-shake-speed*` = 2, `*joypad-shake-speed*` = 2 (used for shake detection; no stick dead zone found).
- `*player-pickup-plank-extra-dist-threshold*` = 0.1.

## E. Still not found
Ground speeds, sprint stamina, capsule sizes, stance times, vault/clamber heights, dive values, camera offsets/FOV. Searched all files whose names mention player, camera, joypad, traversal, jump, health, aim, stat, anim-settings. `player-anim-settings.bin` (1.5 MB) and `player-aim-settings.bin` have no simple top-level floats (all structs); not yet decoded.

## F. What would unlock most of the rest (needs your decision)
The remaining step is to decompile ~15 specific functions (the readers of the structs in section B) so each float can be given its field name. I could not do it without Ghidra:
- `javaw.exe` is running (probably Ghidra with project `decoding` open, so headless access is locked), and
- copying the project to work on it is **2.6 GB**, above your "ask before more than a few hundred MB" rule.
Options: (1) you close Ghidra and I run headless on the live project (no copy), (2) you allow a 2.6 GB copy, or (3) you run a small export script I write (like your other `Export*.java`) for these addresses: 140e21d20, 140e1e290, 140c5fc50, 140bbbd40, 140b6fcb0, 140d95e40, 140b18bd7, 140e04ae0, 140e04d00, 140e08780, 140da81c0, 140ba3d60. Option 3 is the lightest.

Tools added: `dc_tools\` (exeidx.py, dcidx.py, attr.py, scanall.py, structs.py, topvals.py, fdis.py, pe.py).

---
# Round 3: headless decompile of the settings readers (done; Ghidra can be reopened)
Decompiled 21 functions with `ExportSettingsReadersV1.java` into `settings_readers\` (read-only, project unchanged).

## Confirmed from the decompiled code
- **`FUN_140e04ae0` (and five siblings at 140e04d00, 140e059a0, 140e05b40, 140e069d0, 140e06b90)** are the landing/edge "can this landing type happen" tests. Each takes the player's velocity, projects it onto a direction, takes the absolute value and compares it with the two floats of `*player-land-speed-threshold*`: **`pfVar5[0]` = 3.1 and `pfVar5[1]` = 2.0**. The state flags checked beside them decide which of the two applies. So both numbers are **speed thresholds** (confirmed by use; unit m/s is a guess because the engine unit is not stated). The test also needs a valid edge from `EdgeChecker::CalcBestEdge`.
- **`GroundLanding::DoLand` (140e08780)** picks the landing clip by hashed state name (`fall-land-roll-run`, `fall-land-roll-hard-run`, ...), then looks up `fall-recovery` (hash 0x7f05e9e4af6f04dc, data in `player-upgrade-settings.bin`) and wade/water landing (`wade-land`). No hard-coded fall-height number appears in it, so **fall height thresholds are not in this function**; they are either in a data struct read by a deeper caller or a velocity test elsewhere.
- **Health selection (`FUN_140da81c0`)** only picks one of two `player-health-settings` structs by a flag (two hashes: 0x3e60a6ff... or 0x5e8d6cbf...). No numbers in code. Both structs contain the same values (1, 100, 0.5, 0, 0.33, 0.5, 0.35, 0.5).
- **Roll (`FUN_140b6fcb0`)** only selects which roll settings symbol applies (normal, sprint, mp, djinn) by state. Which float is roll speed is therefore still **unverified**.

## Not resolved
The jump, evade-jump, prone, bipod and zipline structs are passed through to deeper handlers (e.g. `FUN_140e02270`, `mv_jump_140e163c0`, `FUN_140c5fc50` consumers). The field-to-meaning mapping needs those handlers decompiled too. Next batch candidates (if you want me to continue): 140e02270, 140e163c0, 140dfd1b0, 140dfd630, 140e00000-range jump handler vtable targets.
No gravity, take-off speed, coyote time, stamina, capsule, stance time or camera value was found in these 21 functions.

---
# Round 4: jump handlers (26 more functions in `jump_handlers\`)
- **Confirmed:** the player jump is **target-based, not a fixed take-off speed.** `JumpTargetVelocity::GetJumpVelWS` (jump-target.cpp) takes a target point and asks a solver (vtable call, then `FUN_140ddf9d0`) for the velocity needed to reach it; the jump handlers (`mv_jump_140e163c0`, `FUN_140e02270`, `FUN_140e19760`, `FUN_140e19df0`, `mv_jump_140e17e00`) rate candidate targets/edges and pass the settings structs to probes. So "take-off speed 5.5 m/s" has no single counterpart: the speed depends on the distance and height of the chosen target. (Mechanism details beyond that: guess.)
- **Not found:** a gravity constant, coyote time or jump buffer in any of the 26 functions (no "gravity" string, no matching constant). Float constants in them are probe parameters (0.1, 0.3, 0.4, 0.5, 1.5, 2.5, 3, 4, 10) and SIMD splats; I could not tie them to named fields without more work, so I am **not** reporting them as values.
- The `*player-jump-settings*` floats (0.25, 2.4, 0.25, 1.24, 1.6, 2.4, 1.5, 3, 3, 5, ...) are therefore most likely **search/probe ranges for target selection** (guess), not physics.
- Recommendation for the Bevy prototype: keep your own gravity/take-off numbers as `placeholder (measure)`; measure jump height/distance in game.
