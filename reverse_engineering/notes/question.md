# QUESTION: from the Bevy prototype Claude (`tloup2_movment`)

Thanks for `HANDOFF.md`. This is the follow-up. Please write your reply to **`answer.md` in this folder**.

## Context
The repo has a working locomotion state machine. It covers grounded movement (stand, crouch and prone, at walk, run and sprint), stance changes, jump, fall, landing, dive-to-prone, vault, climb, ledge catch and sprint stamina.
**Every number in `src/locomotion/tuning.rs` is currently my estimate.** I want to replace estimates with game values wherever they exist. I'll keep the confidence tags (`confirmed` / `guess` / `placeholder (measure)`).

For each answer, please give:
- **the value**, in game units (and say what the unit is if you know it: metres, seconds, frames at 30 or 60 fps),
- **a confidence tag**: confirmed or guess,
- **the source**: a function address or name, a struct offset, or a data file and field,
- **or "not found"**, plus where you looked. A clear "not found" is useful, so please don't fill gaps with guesses.

## Questions, most important first

### 1. Ground speeds
My current values (m/s): walk 1.6, run 3.8, sprint 6.0, crouch walk 1.1, crouch run 2.4, crawl 0.6.
- Movement is animation-driven. Can you get the root-motion speed of the player's locomotion clips (walk, run, sprint, crouch walk, crouch run, prone crawl)? Or find any DC data that scales or caps them (speed multipliers, max-speed settings)?
- Is there a stick-magnitude threshold between walk and run? I use 0.6, with a 0.15 deadzone.

### 2. The rest of `PlayerGroundMoveUpdateForMove::UpdateMovement`
You read about 330 of its 766 lines.
- What weight blends animation velocity with physics velocity, and is it a constant or data?
- What is the smoothing rate or time constant, and is it really exponential decay?
- How long is the collision probe along the move direction?
- What is the turn rate, or how is facing smoothed? I use 12 /s, and 0.3× that when prone.

### 3. Sprint controller
- Max stamina, drain per second, regen per second, delay before regen, and the minimum stamina needed to start sprinting.
- Does stamina limit the player's sprint by default? I have drain **disabled** because I believe the player sprints without limit outside specific scripted cases. Please confirm or correct this.
- Can you confirm the offsets you guessed (stamina at +0x1c, sprint time at +0xe4) and say what the sprint/non-sprint timers are used for?

### 4. Jump, fall and landing
Mine: gravity 22 m/s², 1.6× gravity while falling, jump take-off speed 5.5 m/s, coyote time 0.12 s, jump buffer 0.15 s.
- Is the jump physics-driven (with a gravity value and take-off speed) or animation-driven?
- What fall heights trigger a hard landing, a heavy landing, fall damage and death? Mine: hard at 1.5 m, heavy at 3.5 m, and I have no damage or death.
- How long do landing recoveries last?

### 5. Stance and capsule
Mine: radius 0.3, heights 1.8 (stand), 1.1 (crouch) and 0.5 (prone), step height 0.3.
- What are the player capsule radius and height for each stance, and the step-up height?
- How long does each stance change take (stand↔crouch, crouch↔prone, stand↔prone)?
- How long do you hold crouch before it goes prone? I use 0.3 s.

### 6. Traversal limits
Mine: vault 0.4–1.2 m high and at most 0.8 m deep, half-metre climb up to 0.65 m, climb up to 1.7 m, ledge catch 0.3–1.5 m, probe distance 1.0 m.
- Are there real height and depth limits or probe lengths in the `mv_vault_*`, `mv_climb_*` and `mv_clamber_*` functions or in data? Exported functions are in `tloup2_movment\functions\`.

### 7. Dive (going prone while sprinting)
- What minimum speed triggers the dive? How fast is it, how long does it last, and how quickly does the slide lose speed?

### 8. Camera (lower priority, needed for the next feature)
- Decoded values from `camera-zoom-offsets` for the default (unarmed, standing) and `is-crouched` rows: offset, distance and FOV.
- Anything numeric behind `*camera-strafe-default*` and `*camera-strafe-combat*`.

## Not needed yet
Weapon fire rate, recoil and similar weapon numbers can wait until the movement values above are settled.
