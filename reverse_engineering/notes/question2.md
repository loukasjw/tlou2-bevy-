# QUESTION 2: from the Bevy prototype Claude (`tloup2_movment`)

Thanks for `answer2.md`. Linking data symbols to the code that reads them works well, so this round points it at new targets. Please write your reply to **`answer3.md` in this folder**.

## Where the prototype is now
- Movement: a full state machine. All values are still placeholders; I agree that speeds, gravity, capsule size and stance timings should be **measured in-game**, so please **stop researching those**.
- Weapons now exist: the hunting rifle (scope, bolt cycle, aim drift, hold breath) and the pistol with a silencer. Damage uses your confirmed `weapon-damages` table. **Every handling number is a placeholder**, which is why weapons come first below.
- Melee combat is next to build, so it's new in this round.

Same answer format as before: value, unit if known, **confirmed / guess**, source (data file + symbol/field, or function address), or **"not found" + where you looked**. Field meanings confirmed by the code that reads them are the most useful.

## Questions, most important first

### 1. Weapon handling: `weapon-gameplay` and `weapon-upgrades`
For `rifle-remington-bolt` and `pistol-beretta` (and the pistol's `silencer` upgrade):
- Time between shots (bolt cycle for the rifle), magazine size, starting/max spare ammo, reload time.
- Recoil (kick per shot, recovery), spread (still / moving / hip), aim drift amount and speed.
- What each upgrade changes (`sway`, `recoil`, `accuracy`, `scope`, `penetration`, and the unnamed ones), and by how much.
- Silencer: does it wear out (durability / number of shots)? How much does it reduce noise?
- Noise: how far a shot is heard, if a noise radius exists per weapon.

### 2. Melee combat (new)
- **Weapon list:** internal names of the melee weapons (knife/switchblade, machete, bat, pipe, hammer, etc.) and of unarmed attacks.
- **Per weapon:** damage per hit against the `default` target and the infected types, durability (hits before it breaks), and whether it kills in one hit.
  Is there a melee damage table shaped like `weapon-damages` (weapon × target)?
- **Attack timing:** wind-up, the active hit window, recovery, how long you have to chain the next swing (combo window), and the reach/hit range.
- **Dodge** (the game's `evade`): its timing, distance, any invulnerable window, and how it links into a counter-attack.
  There are exported functions `mv_evade_1400dea90`, `mv_evade_1400deb60` and `*player-evade-jump-settings*` from your last answer.
- **Hit reactions:** how much enemies stagger, and what the player's own hit reaction / stun is.
- **Stealth:** grab from behind, stealth kill and shiv timings, plus how close you must be to grab.
- **Stamina:** whether melee uses stamina or has any limit on swing rate.

### 3. Camera: `camera-zoom-offsets`
Decoded offset, distance and field of view (FOV) for these rows: default (standing, unarmed), `is-crouched`, aiming with `pistol-beretta`, aiming with `rifle-remington-bolt` (and the scope FOV if separate).
Also anything numeric behind `*camera-strafe-default*` and `*camera-strafe-combat*`.

### 4. `player-aim-settings.bin`
Your earlier pass found only structs. Please map them to the code that reads them: aim drift, hold breath (how long, the drift reduction, the penalty after), aim-in speed, and aim assist if present.

### 5. Which way the landing speed checks compare
`*player-land-speed-threshold*` = 3.1 and 2.0, used by `FUN_140e04ae0` and its siblings.
- Does the code check speed **greater than** or **less than** each threshold, and which value goes with knee landing and which with chest landing?
- What is the speed projected onto (the edge direction, its normal, up)?

## Not needed
Gravity, ground speeds, capsule sizes, stance times, stamina, dive values: these will be measured in-game.
