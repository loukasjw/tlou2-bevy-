# ANSWER to question.md

Short version: **I could not find any of your locomotion tuning numbers as readable data.** What I did find is listed below, with confidence tags. Nothing here is invented. Where it says "not found" I list where I looked, so you do not need to repeat it.

Why so little: locomotion speed is not a constant. It comes from animation clips (motion matching) and from nested data structs whose field names are hashed. The clip files (`actor97/*.pak`, anim data) are not parsed, so root-motion speed cannot be measured from the files yet. **Measuring in-game is the realistic route for speeds, fall heights and timings.**

Method used this round: re-read all 766 lines of `PlayerGroundMoveUpdateForMove::UpdateMovement` (`tloup2_movment\functions\...140c3e5f0.c`), read float constants straight out of `tlou-ii.exe` (virtual address -> file offset), disassembled the helper functions the decompile did not export, extracted 83 more `dc1` data files (`anim-player/*`, collision settings, move-performances, climb anims, npc-locomotion) and scanned them with the FNV-1a name dictionary.

---

## 1. Ground speeds: NOT FOUND
- Looked in: `dc1/anim-player/player-move.bin`, `player-motion-match-locomotion.bin`, `player-strafe.bin`, `player-crouch.bin`, `player-prone.bin`, `player-parametric-move-sets.bin`, plus the exe strings.
- What these files contain (confirmed): animation state-machine scripts. Resolved names include `idle-run-f/b/l/r/fl/fr/bl/br`, `run`, `sprint-run`, `crouch-move-enter`, `player-anim-speed-mult`, `calc-new-phase-from-speed-xz`, `get-state-align-speed-from-tree-xz`, `motion-matching-suppress-change-stick-spring-constant`, `motion-matching-suppress-change-max-turning-angle`. The numeric values sit in fields I cannot name.
- Confirmed facts that matter: there is an anim playback **speed multiplier** (`player-anim-speed-mult`), and run uses an 8-direction blend (`idle-run-*`). That supports "speed = clip root motion x multiplier". The multiplier value was not found.
- Root-motion speed of each clip: **not found**. It needs the animation pak files parsed.
- Walk/run stick threshold and deadzone: **not found.** A function name `get-joypad-stick-intensity` exists in `player-misc.bin`, but no threshold was read.
- Do NOT use `mv_sprint_1405903a0` for the player. Its strings ("Max Gallop Speed", "Spur Accel") show it is the **horse** debug print.

## 2. `UpdateMovement` (fully read)
Everything below is **confirmed from the code** unless marked guess. Constants were read from the exe.

- **Branch:** the function reads `w = FUN_140c36cb0(player)` (an animation-state query that reads a value through the animation system, so `w` is **data/animation driven, not a constant**). If `w < 1.0` it takes the "blend" path; otherwise the other branches (scripted/special states are keyed by ~14 state-name hashes; some of them just `return`).
- **Blend:** blended velocity = `(1 - w) * (animation displacement / dt) + w * (current physics velocity)`. So `w` is the physics weight. `dt` is the frame delta. **Confirmed (code), the meaning of `w` = guess.**
- **Smoothing rate:** the smoothing uses a closed form with `exp(-k*dt)` (so yes, exponential decay; it looks like a critically-damped form, **guess**). `k` comes from a remap helper `FUN_140c36110`. I disassembled it: it linearly maps `w` from `0 .. 0.25` onto `20 .. 1000` (per second) and clamps. So **k = 20 /s when w = 0 (time constant 0.05 s) rising to 1000 /s (effectively no smoothing) when w >= 0.25.** Constants 0.25, 20.0, 1000.0 are **confirmed** (exe addresses 142af9618, 142affe74, 142af9630). That they are a smoothing rate is a **guess** from the formula shape.
- **Second remap:** `FUN_140c36110(x=0.7, 0.9 -> 0, 1.0)` applied to a float read at player offset +0x19c. It blends something between 0 and 1 over the range 0.7..0.9. What +0x19c is: **not found**.
- **Collision probe:** the probe origin is advanced by a velocity times a small multiplier, and there are multiplier constants **3.0** (142affe50) and **4.0** (142b05918) and **0.5** (142af9620) in the sweep set-up. I could not turn these into a length in metres. **Not found.**
- **Turn rate / facing smoothing:** not in this function. **Not found.** Related names exist in data: `motion-matching-suppress-change-max-turning-angle`, `...stick-spring-constant`, `InvestigateTurnSpringAction`.
- Tiny constant `0.001` (142affde8): the function ends by calling a virtual method when `w > 0.001`. Purpose: **guess** (a "moving" test).

## 3. Sprint controller: NOT FOUND
- Exported only `PlayerSprintController::IsFleeing` (140cd34e0). No stamina max, drain, regen, delay or minimum.
- Searched the exe strings for `stamina`, `sprint speed`, `max speed`: no matches. There is no sprint-related tuning string for the player.
- Your belief ("player sprints without limit outside scripted cases"): **not confirmed and not contradicted.** I found no stamina value, which is weak evidence only (**guess**) that no stamina limit is data-tunable. The earlier offsets (+0x1c, +0xe4) remain **guess**; I did not re-verify them.
- Lead: `dc1/anim-player/player-parametric-move-sets.bin` (1,174 bytes, contains `sprint-run`) is small enough to decode by hand.

## 4. Jump, fall, landing: only structure found
- Confirmed names (from `anim-player/player-fall.bin`): `PlayerFallController`, `PlayerFallMoveAction`, `PlayerGroundLandingAction`, `PlayerFallToGroundDie`, `fall-intro`, `fall-die`, `fall-to-ground-die`.
- Confirmed landing tiers by clip name: `fall-land-soft-run`, `fall-land-medium-run`, `fall-land-roll-run`, `fall-land-roll-hard-run`, `fall-land-early-stumble-run`, `fall-land-extra-hard-idle`, `fall-land-extra-soft-run`, `fall-land-prone`. So the real game has **at least soft / medium / roll / hard-roll / extra-hard tiers and a fall death** (confirmed they exist). My suggestion: you may want more than two landing tiers.
- Jump clip names: `jump`, `jump-long`, `run^dive-jump`, `melee-jump-from-run`, `fail-move-jump`. Whether the jump is physics- or animation-driven: **not found.** (Landing classes like `GroundLanding::DoLand` exist in `functions\`, but gravity or take-off speed were not read.)
- Gravity, take-off speed, coyote time, jump buffer, fall heights, landing durations: **all not found.** No gravity string exists in the exe other than unrelated ones (weapon strap gravity, bow arc gravity).

## 5. Stance and capsule: NOT FOUND
- `dc1/characters-collision-settings.bin` looked promising but is **hit-boxes** (`targHead`, `targChest`, `base-female-collision`, ...), not the locomotion capsule. Capsule radius/height/step height: not found.
- Stance timings: not found. Names only: `crouch-move-enter`, `crouch-idle`, `disable-crouch-changes`, `CharacterProne*` actions, `player-is-crouched?`, `player-is-prone?`. A prone **hold time** was not found.

## 6. Traversal limits: only hints
- Confirmed state names: `clamber-begin`, `clamber-low-begin`, `vault`, `ledge-grab`, `CharacterLedgeClimbUp`, `CharacterLedgeDrop`, `CharacterLedgeDropHeightScale`, `drop-height`.
- Clip names that hint at heights (**guess**, naming only): `crouch-jump-fwd-1m`, `ellie-idle-clamber-up-entry-1m`, `s_rope-climb-2-75m`.
- Real min/max heights, depths and probe lengths: **not found.** I did not read the `mv_vault_*` / `mv_climb_*` / `mv_clamber_*` bodies line by line this round. They are unnamed (`mv_*` are keyword matches, not named functions), so I judged it low-yield. If you want, name the 3-4 you suspect and I will read them.

## 7. Dive: NOT FOUND
- Confirmed: a dive transition exists (`run^dive-jump` in `player-jump.bin`, `swim-surface^dive-fw` for water). Minimum speed, dive speed, duration, slide deceleration: not found. `player-slide.bin` exists (36 KB) but slide values were not decoded.

## 8. Camera: NOT DECODED
- `camera-zoom.bin` has 146 top-level records of type `camera-zoom-offsets` / `camera-dof-settings`. Confirmed that records are chosen by conditions (`weapon-id`, `is-crouched`, `weapon-anim-type`, plus special cases like `Swim`, `Slide`, `Bipod`, `GunMoveHorse`). The offset/distance/FOV numbers are inside structs I cannot name yet, so **no values**.
- `*camera-strafe-default*` / `*camera-strafe-combat*`: strings exist in the exe; no numeric values found.
- One structural hint for design: the camera offset really is **per weapon and per stance** (confirmed), so a data table keyed by (weapon, stance) matches the original.

---

## What WOULD move this forward (in order of payoff)
1. **Measure in game:** record video (60 fps), count frames over a known distance for walk/run/sprint/crouch/crawl speeds; drop from marked heights to find landing tiers and death height. This answers sections 1, 4, 5, 7 faster than any more reverse engineering.
2. **Decode one DC struct type properly.** The same trick that worked for weapon-damages (find type hash -> field layout) would unlock camera, parametric move sets and fall data. It needs the exe's DC reader to name fields; that is a larger task.
3. **Parse the animation paks** (`actor97/*.pak`) for root-motion speed. Largest effort, only worth it if you want exact clip speeds.

## Tools and files (this folder)
- `dc_tools\` (hash dictionary, dumpers), `extracted\bin\dc1\anim-player\` (all player animation scripts), `listings\`.
- Also learned: `PYTHONPATH=. python -P script.py`; do not name a script `dis.py` (it shadows a stdlib module).
