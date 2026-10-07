# ANSWER 3 (reply to question2.md)

Short version: **weapon fire rate, magazine, reload, recoil, spread, aim drift, noise radius, silencer durability amount, melee timings, dodge timings, camera offsets and `player-aim-settings` were NOT found.** What I did find: how the weapon data is organised and read (confirmed), a per-weapon float table whose field meanings are partly guessed, the melee weapon list, and the exact direction of the landing checks. Nothing below is invented; every number is read from a data file or from the decompiled code.

Method this round: found the code that reads `weapon-gameplay.bin` by linking its symbol hashes to the exe, decompiled 58 small accessor functions (`weapon_accessors\`), and read the struct offsets they use against the same offsets in the data file (the data structs are memory images, so file offset = C++ offset). Also headless-decompiled the silencer/landing functions (`weapon_readers\`, `settings_readers\`).

---

## 1. Weapon handling

### 1a. How the data is organised (confirmed)
- `dc1/weapon-gameplay.bin` has a `map` (172 entries) from weapon id to a weapon-definition struct. Lookup function: `FUN_141163da0` (binary search by id). The struct holds pointers: `+0x10` = main handling struct (type hash `05dffdf09acc3d5e`), `+0x28`, `+0x30`, `+0xa8`, `+0x88`.
- 58 small getters (`14115dbe0 ... 141165b8a`, now in `weapon_accessors\`) read fields from it. Most do: **"ask the weapon instance for an override of kind N; if none, return the data-file value."** The override kinds seen: 8, 10, 0xd, 0xf (via `mv_weapon_14115d670`, `141f31490`, `14115d5e0`). Confirmed meaning: **upgrades and runtime state override these data fields by index**, which is how `weapon-upgrades` changes handling.
- `weapon-upgrades.bin`: each weapon has named upgrade slots (`rifle-remington-bolt`: penetration, sway, scope, accuracy + 6 unnamed; `pistol-beretta`: silencer, recoil, sway, accuracy + 2 unnamed). Per-level amounts were **not decoded**.

### 1b. Per-weapon float table (VALUES CONFIRMED, MEANINGS MOSTLY GUESS)
Values read at the struct offset used by the named getter. Units are not stated in the data.

| Offset (getter) | Remington | Tommy | Beretta/Glock | MPX5 | Pump | Lever | Bow | My reading |
|---|---|---|---|---|---|---|---|---|
| 0x18 (1411619b0) | 0.13 | 0.13 | -1 | 0.13 | -1 | 0.13 | -1 | unknown. Set only on rifles; -1 = "none" |
| 0x6c (141160fa0, override 10) | 0.9 | 0.9 | 0.4 | 0.5 | 0.4 | 0.75 | 0.4 | **guess:** a time in seconds (raise/aim-in); longer for rifles |
| 0x70 (141163640, override 10) | 0.4 | 0.4 | -1 | -1 | -1 | -1 | 0.4? (bow -1) | **guess:** bolt-action only (Remington, Tommy, M24) |
| 0x80 (141162850) | 1500 | 1500 | 60 | 1000 | 25 | 250 | 500 | **guess:** maximum range |
| 0xa0 (141161030, override 8) | 50 | 50 | 60 | 55 | 54 | 54 | 65 | **guess:** a camera FOV in degrees when aiming |
| 0xa4 (1411610a0, override 8) | 0.85 | 0.8 | 1 | 1 | 1 | 1 | 0.55 | unknown multiplier (<1 on bolt rifles and bow) |
| 0xa8 (1411633d0, override 8) | **35** | **7.15** | -1 | 53.13 | -1 | 54 | 10 | **guess:** scoped FOV in degrees (see below) |
| 0xac (1411634b0, override 8) | 0.45 | 0.225 | 1 | 0.62 | 1 | 1 | 0.25 | unknown multiplier on scope |
| 0x260, 0x264 (141160d00, 141164520) | 1.15, 1.35 | same | 1, 1 | 1.15, 1.35 | 1, 1 | 1.15, 1.35 | 1, 1 | unknown multipliers |
| 0x270 (141160c20, override 0xd) | 0.9 | 0.9 | 1 | 1 | 1 | 1 | 0.9 | unknown multiplier |

Why I believe 0xa8 is a scope FOV (guess, but with a numeric check): 53.13 = 2*atan(1/2) exactly, 7.15 = 2*atan(1/16) exactly. So they look like FOV for 2x and 16x zoom relative to a 90 degree field. Remington 35 degrees would then be about 3.2x zoom. If so: **Remington scoped FOV = 35 degrees (guess), pistol: none (-1).** Treat as a guess until checked in game.
Also `rifle-tommy` shares the Remington's bolt-action flags, so "Tommy's rifle" is the same bolt-action family (confirmed by identical 0x18/0x70/0x6c).

### 1c. Not found (where I looked)
- **Time between shots / bolt cycle, magazine size, spare ammo, reload time, recoil, spread, aim drift:** not in the `+0x10` struct fields that have getters, and not as named fields anywhere in `weapon-gameplay`, `weapon-upgrades`, `weapon-damages`, `weapon-sway-settings` (45-86 bytes, empty), `weapon-kickback-defs` (empty), the weapon overlay files (these only swap animation clips: `anim-overlay-set`, `ride-horse-mm`, clamber/evade clip names).
- Debug strings confirm such systems exist (recoil angle/tracker speed, camera kickback, reticle kickback, "Visualize Bullet Spread", `player-get-sway-breath-meter`, `aim-hold-breath`, `get-weapon-process-clip-size`) but I could not reach their data: those script functions are not registered by name pointer and have no direct code references.
- **Silencer:** confirmed it has **durability**: script functions `player-get-weapon-silencer-health` (140cb3c60) and `player-get-current-weapon-silencer-health` (140cb3a00) return an integer from the silencer part object (offset +0x844 of the instance, `FUN_141186d50`). The **maximum, loss per shot and noise reduction were not found.** `PlayerSilencerController::Init` (140cc1d70, player-silencer.cpp) only allocates its list.
- **Noise radius per weapon:** not found.

---

## 2. Melee

- **Weapon list (confirmed, internal names in `weapon-damages`/`weapon-gameplay`):** unarmed `abby-unarmed`; knives `knife-survival-mel-player`, `switchblade`, `switchblade-dina-player`, `dagger-scar-lev-player`; `machete`, `machete-scar`, `machete-wood`; `hatchet`, `hatchet-black`, `hatchet-scar`; `axe`, `axe-scar`, `axe-fireman`; `baseball-bat`, `baseball-bat-metal`; `lead-pipe`, `pipe-wrench`, `crowbar`, `two-by-four`; `hammer-modern`, `hammer-scar`, `sledgehammer`; `club-1h-police`, `club-1h-scar`, `club-2h-scar`; `pickaxe-player`, `pickaxe-scar`; `scythe-scar`; thrown/improvised `bottle`, `brick`.
- **Damage:** the melee rows in `weapon-damages` are **identical for almost every melee weapon**: `infected-bloater` 8, `infected-runner` 12 (knives only), and two target types with 60 and 8. There is **no `default` row** for melee. So **per-weapon melee damage is not in this table** (confirmed). The real melee damage must come from the melee move data (`dc1/melee/player-standing/*.bin`, e.g. `player-evades.bin` 32 KB, `player-rifle.bin`), which I have **not decoded**.
- **Durability, one-hit kills, attack timings (wind-up / hit window / recovery / combo window), reach, dodge timing and invulnerability, hit-reaction stagger, grab and stealth-kill timings, melee stamina: all not found.** Only names were seen: `CharacterMeleeWeaponAdjust::DamageSweep::HandleDamageSweeps` (140a46b30, a per-frame weapon sweep damage test), `melee-state-prone-evade`, `PlayerEvadeController::*`. I did not decompile them this round.
- Evade: `*player-evade-jump-settings*` is a jump-target settings struct (same layout as `*player-jump-settings*`), so it governs jumping off cover, not the dodge timing.

---

## 3. Camera (`camera-zoom-offsets`): NOT DECODED
- Structure only (confirmed): 146 records selected by conditions (`weapon-id`, `weapon-anim-type`, `is-crouched`, plus Swim/Slide/Bipod/GunMoveHorse cases). The `rifle-remington-bolt` row has a `weapon-id` condition and an `is-crouched` sub-condition. No offset/distance/FOV numbers were read. The only FOV-like values found are in section 1b (0xa0 = 50 for the Remington, 60 for the Beretta; both guesses).
- `*camera-strafe-default*` / `*camera-strafe-combat*`: strings only; no numeric value found.

## 4. `player-aim-settings.bin`: NOT DECODED
All structs; no function reading it was identified (no code references to its symbol hashes in named functions). Aim drift, hold breath, aim-in speed, aim assist: not found. Related names exist (`player-get-sway-breath-meter`, `player-get-sway-breath-state`, `aim-hold-breath`, `player-disable-weapon-sway`) but their data was not located.

---

## 5. Landing speed checks (CONFIRMED from the decompiled code)
`*player-land-speed-threshold*` = { **3.1**, **2.0** }. All six functions have this shape:
- `speed` = a non-negative speed measure of the player:
  - `FUN_140e04d00`: **horizontal speed** = sqrt(x^2 + z^2) of the velocity vector passed in (y ignored).
  - `FUN_140e04ae0`, `FUN_140e059a0` and siblings: **|player velocity projected onto a direction built from the player's orientation|** (velocity read at player offset +0x5a0; the direction is a rotation of a constant by the player's orientation, so most likely the facing/forward axis; the exact axis is a guess).
- The landing type (a virtual method of the landing class) is allowed only if both of these hold:
  - if landing flag A (vtable +0xa8) is true, then **speed < 3.1** (strictly less);
  - if landing flag B (vtable +0xb0) is true, then **speed >= 2.0** (at least).
  So 3.1 is an **upper** limit for the A-type landing and 2.0 is a **lower** limit for the B-type landing. 140e04d00 also requires a vertical test to pass first (a scaled value >= a constant from the exe; value not extracted).
- Which flag is knee and which is chest: **not confirmed.** The functions sit next to `KneeLanding::DoLand` (140e04270) and `ChestLanding::DoLand` (140e05ef0); the pair 140e04ae0/140e04d00 precedes the knee code and 140e059a0/140e05b40 the chest code (guess by address). The A/B flags are virtual methods I did not read.

---

## Honest status and next steps
Of the 5 groups asked: groups 1 (partly), 2 (names only) and 5 (done) have results; 3 and 4 are not found.
Most useful next steps if wanted:
1. Decompile the melee damage path (`HandleDamageSweeps` 140a46b30 and the `melee/player-standing/*.bin` readers) to get melee hit data.
2. Find how scripts like `aim-hold-breath` read their data (script bytecode in `anim-player/*.bin` and `modules.bin`).
3. Measure fire interval, reload, recoil and magazine in game (the structs above show no numeric field for them).

Tools/files: `weapon_accessors\` (58 getters), `weapon_readers\`, `settings_readers\`, `jump_handlers\`, `dc_tools\` (plus `rwalk.py`, `accmap.py`, `xref.py`, `ptrx.py`, `callers.py`, `regname.py` in my scratch; copied below if needed).
