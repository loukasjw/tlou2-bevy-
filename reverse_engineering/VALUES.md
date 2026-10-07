# VALUES: every number found, in one place

Tags: **C** = confirmed (read from game data/code), **G** = guess. Units are not stated in the game files; units shown are guesses unless noted.

## 1. Weapons: identities and damage (C)
- Hunting rifle `rifle-remington-bolt` (same handling family: `rifle-bolt-m24`, `rifle-tommy`). Pistol `pistol-beretta` (Abby: `pistol-glock`). Silencer = upgrade of the pistol.
- Damage record per (weapon, target) = (int, float a, float b). Meaning of the int: damage per hit (G). Floats: falloff start/end distance (G); -1 = no falloff.

| Weapon | Target | int | a | b |
|---|---|---|---|---|
| rifle-remington-bolt | default | 80 | 150 | 300 |
| rifle-remington-bolt | infected-bloater | 70 | -1 | -1 |
| rifle-remington-bolt | 2 unnamed target types | 80 | 20 | 20 |
| pistol-beretta | default | 34 | 30 | 30 |
| pistol-beretta | infected-runner | 30 | -1 | -1 |
| pistol-beretta | infected-bloater | 40 | -1 | -1 |
| pistol-beretta | 2 unnamed target types | 34 | 15 | 15 |
| rifle-tommy | default | 100 | 30 | 30 |
| pistol-glock | default | 34 | 30 | 30 |
| shotgun-remington-pump | default / clicker | 101 / 151 | 8 | 8 |
Full table: `data/weapon-damages.csv`.

## 2. Weapon handling floats (values C, meanings G)
Read at struct offsets from `weapon-gameplay`; the Remington/Beretta columns:

| Field (offset) | Remington | Beretta | Likely meaning (G) |
|---|---|---|---|
| 0x6c | 0.9 | 0.4 | time in seconds (raise/aim-in) |
| 0x70 | 0.4 | -1 | bolt-action only |
| 0x80 | 1500 | 60 | maximum range |
| 0xa0 | 50 | 60 | aim camera FOV, degrees |
| 0xa4 | 0.85 | 1 | multiplier |
| 0xa8 | **35** | -1 | **scoped FOV, degrees** (53.13 and 7.15 on other guns equal 2x/16x zoom exactly, which supports this) |
| 0xac | 0.45 | 1 | multiplier on scope |
| 0x260 / 0x264 | 1.15 / 1.35 | 1 / 1 | multipliers |
| 0x270 | 0.9 | 1 | multiplier |
| 0x18 | 0.13 | -1 | unknown (set only on rifles) |

Silencer (C): has an integer **health/durability** (script functions `player-get-weapon-silencer-health`). Maximum, loss per shot and noise reduction: not found.

## 3. Melee weapon names (C)
Unarmed `abby-unarmed`; knives `knife-survival-mel-player`, `switchblade`, `switchblade-dina-player`, `dagger-scar-lev-player`; `machete` (+`-scar`, `-wood`); `hatchet` (+`-black`, `-scar`); `axe`, `axe-scar`, `axe-fireman`; `baseball-bat`, `baseball-bat-metal`; `lead-pipe`, `pipe-wrench`, `crowbar`, `two-by-four`; `hammer-modern`, `hammer-scar`, `sledgehammer`; `club-1h-police`, `club-1h-scar`, `club-2h-scar`; `pickaxe-player`, `pickaxe-scar`; `scythe-scar`; throwables `bottle`, `brick`.
Melee damage rows are identical across weapons (bloater 8, runner 12 for knives): per-weapon melee damage is NOT in the damage table. Melee timings, durability, dodge: not found.

## 4. Traversal limits (C values, units G) - `player-traversal-settings`
| Name in data | Value |
|---|---|
| jump-backwards-angle-threshold | 60 (deg) |
| jump-backwards-seconds-threshold | 0.5 (s) |
| jump-to-wall-angle-threshold | 40 (deg) |
| max-dyno-jump-angle-to-wall | 30.5 (deg) |
| evade-off-edge-grab-height | 2 (m) |
| run-off-edge-grab-height | 8 (m) |
| up-back-dyno-min-xz / max-xz | 0.95 / 2.05 (m) |
| up-back-dyno-min-y / max-y | 0.45 / 1.55 (m) |
| edge-look-away-spring / left-right-spring | 6 / 9 |
| water-ladder-depth | 2.7 (m) |
These are edge-grab/jump-probe limits, not the vault or clamber height limits (not found).

## 5. Landing (C)
`*player-land-speed-threshold*` = {3.1, 2.0}. A landing type is allowed only if: flag A true -> speed **< 3.1**; flag B true -> speed **>= 2.0**. Speed is horizontal speed or the player's velocity projected on an axis built from the player's orientation (axis: G). Which flag is knee vs chest: not confirmed.
Landing animation tiers that exist (C, by clip name): soft, medium, roll, hard-roll, early-stumble, extra-soft, extra-hard (idle), prone landing, plus fall death. Thresholds in height: not found. Recovery time is data-driven (`fall-recovery`, upgradeable).

## 6. Jump (C structure, G numbers)
Jump velocity is solved to reach a chosen target point (`JumpTargetVelocity::GetJumpVelWS`), so there is no fixed take-off speed. `*player-jump-settings*` floats (0.25, 2.4, 0.25, 1.24, 1.6, 2.4, 1.5, 3, 3, 5, 2.4, 1, 1, 0.3, 0.75, 2.26, 2.7, 0.9, 1.2, 1.2) are most likely target-search ranges (G). Gravity, coyote time, jump buffer: not found.

## 7. Ground movement (C structure)
Animation-driven. Blend: `(1 - w) * (animation displacement / dt) + w * physics velocity`, where `w` comes from animation data. Smoothing: exponential, rate `k` goes 20 to 1000 per second as `w` goes 0 to 0.25 (constants C; "smoothing rate" meaning G). Speeds, turn rate, probe length: not found.

## 8. Other (values C, meaning mostly G)
- Roll settings (second float): normal 4.5, sprint 4.8, mp 5.75, djinn 8 (roll speed in m/s: G).
- Health settings struct: 1, 100, 0.5, 0, 0.33, 0.5, 0.35, 0.5 (100 = max health: G).
- Joypad: shake threshold 1, stick shake speed 2.

## 9. NOT FOUND: measure in-game
Walk/run/sprint/crouch/crawl speeds, stick thresholds and dead zone, sprint stamina, gravity, jump arc, fall damage/death height, capsule sizes, stance change times, vault/clamber limits, dive values, fire interval, bolt cycle, magazine, spare ammo, reload, recoil, spread, aim drift, hold-breath numbers, noise radius, silencer durability maximum, melee timings and durability, dodge timing, camera offsets and FOV per stance, camera strafe values.
