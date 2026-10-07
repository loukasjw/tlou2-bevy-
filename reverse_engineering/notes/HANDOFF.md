# HANDOFF: reverse-engineering findings for the Bevy prototype

Written for the Claude working in this repo (`tloup2_movment`, Bevy 0.19.1, Rust 2024).
The reverse-engineering lives in a separate folder: `C:\Users\louka\coding\ghidra tlou decoding\` (read its `CLAUDE.md` for full background).
Goal here: recreate the **feel** of The Last of Us Part II (movement, over-the-shoulder camera, hunting rifle, silenced pistol).
Do NOT copy or ship Naughty Dog assets (meshes, audio, textures). Use placeholder capsules/boxes. Values below are numbers/behaviour only.

Confidence tags: **confirmed** = read directly from game data/code; **guess** = inferred. Never invent stats.

## 1. What you can use now

### Weapon identities (confirmed)
- Hunting rifle: internal name `rifle-remington-bolt` (bolt-action).
- Pistol: `pistol-beretta` (Ellie). Abby's is `pistol-glock`.
- The silencer is a workbench **upgrade** of the Beretta (`silencer` entry in `weapon-upgrades`), not a separate gun.
- Rifle upgrade entries (confirmed names): penetration, sway, scope, accuracy, plus 6 unnamed. Pistol: silencer, recoil, sway, accuracy, plus 2 unnamed.

### Damage records (confirmed values, meaning partly guess)
Source: `dc1/weapon-damages.bin`. Full table: `ghidra tlou decoding\extracted\decoded\weapon-damages.csv` (240 rows).
Each record = (int, float a, float b).

| Weapon | Target | int | float a | float b |
|---|---|---|---|---|
| rifle-remington-bolt | default | 80 | 150 | 300 |
| rifle-remington-bolt | infected-bloater | 70 | -1 | -1 |
| rifle-remington-bolt | 2 unnamed targets | 80 | 20 | 20 |
| pistol-beretta | default | 34 | 30 | 30 |
| pistol-beretta | infected-runner | 30 | -1 | -1 |
| pistol-beretta | infected-bloater | 40 | -1 | -1 |
| pistol-beretta | 2 unnamed targets | 34 | 15 | 15 |

- The int is probably damage per hit (**guess**). Unit unknown; NPC health not known, so "one-shot kill" cannot be claimed.
- The floats look like falloff start/end distances (**guess**): pistols 30, shotguns 8-10, rifles/bows 150/300, `-1` = no falloff.
- Use these as relative tuning ratios, not absolute truth.

## 2. What is NOT known (measure in-game, do not invent)
Fire rate / bolt-cycle delay, magazine size, reload time, recoil, spread, sway amount, hold-breath behaviour numbers, scope zoom FOV, noise radius, walk/sprint speeds.
These live in nested structs (`weapon-gameplay`, `weapon-upgrades`, `camera-zoom`) whose field types are not decoded yet. Plan: record gameplay and count frames, then put results in `tuning.rs` with a source comment.

## 3. Known behaviour from the executable (from the ghidra CLAUDE.md)
- Ground movement is **animation-driven** (`PlayerGroundMoveUpdateForMove::UpdateMovement`): per-frame animation displacement becomes a velocity (divided by dt), blended with physics velocity by a weight, a short probe along the move direction shortens the step on collision, then exponential-decay smoothing (smoothing style is a **guess**). Only ~330 of 766 lines were read.
- No single "walk speed" constant exists; speeds come from animation clips plus data. Character uses motion matching.
- Sprint controller keeps stamina, sprint toggle, sprint time, non-sprint time (**guess**: stamina at +0x1c, sprint time at +0xe4).
- Camera code exists for: camera sway, reticle sway, scoped FOV, forced FOV limits, `*camera-strafe-default*`, `*camera-strafe-combat*`.
- `camera-zoom.bin` holds `camera-zoom-offsets` and `camera-dof-settings` records selected by conditions such as `weapon-id == rifle-remington-bolt` and `is-crouched`. So aim camera offset varies per weapon and stance (**confirmed** structure; values not decoded).

## 4. Suggested mapping onto this repo
Current design: pure state machine in `src/locomotion/` (`state`, `sensors`, `systems`, `tuning`, plus `sprint.rs`, `traversal.rs`, `motion_model.rs`), ordered Input -> Sense -> Transition -> Movement -> Integrate, driven by `MoveIntent`.
Suggested next features, in order:
1. Over-the-shoulder camera with per-weapon, per-stance offset table (data-driven, mirroring `camera-zoom-offsets`).
2. Aim state + hunting rifle: bolt-cycle delay between shots, sway that reduces while holding breath, scoped FOV. Numbers are placeholders until measured.
3. Silenced pistol: semi-auto, low noise event; silencer as a toggleable modifier component, not a separate weapon.
4. Damage as a per-weapon, per-target-type table with optional distance falloff (shape confirmed by the data above).
Keep all tunables in `tuning.rs`, each tagged `// confirmed:` or `// guess:` or `// placeholder (measure)`.

## 5. Where things are
In `C:\Users\louka\coding\ghidra tlou decoding\`:
- `CLAUDE.md`: full background and gotchas.
- `functions\` + `index.csv`: decompiled functions (camera/weapons export not yet read).
- `extract_dsar.py`: streaming archive lister/extractor (`--list`, `--only`). Needs `pip install lz4`; run `python -P`.
- `listings\`: file lists of the four archives. `listings\MATCHES.txt` = weapon-related files.
- `extracted\bin\dc1\`: 291 weapon/camera data files. `extracted\decoded\weapon-damages.csv`: decoded table.
- `dc_tools\`: scripts to decode the `00CD` data format (FNV-1a 64 name hashes are confirmed).

## 6. Pitfalls
- Archive file names must be paired to entries by MD5(path), not list position (an old bug mislabeled files; fixed).
- Many unresolved names remain as hashes; the dictionary only recovers some.
- Large game archives (`sp-common.psarc` 21 GB) are untouched; do not modify game files.
- Copyright: keep extracted assets out of this repo.
