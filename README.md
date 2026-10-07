# tlou2-movement-bevy

A [Bevy](https://bevy.org) (Rust) reimplementation of *The Last of Us Part II*'s movement, camera and combat systems, built from reverse-engineered tuning data. Early prototype: placeholder capsule and boxes, no game assets.

> Unofficial fan project. Not affiliated with or endorsed by Naughty Dog or Sony Interactive Entertainment. Contains no original game code, models, audio or textures.

## Status

- **Locomotion**: state machine (walk, sprint, crouch, prone, jump, vault/traversal), sensors and a motion model.
- **Weapons**: damage tables and melee logic modules, in progress.
- **Camera**: follow camera, in progress.

## Run

Requires a recent stable Rust toolchain (edition 2024).

```
cargo run
```

The first build compiles Bevy and takes a while. Dependencies are optimized even in dev builds, so later runs are fast.

## Controls

| Action | Keyboard | Gamepad |
|---|---|---|
| Move | WASD | Left stick |
| Walk (half speed) | Left Alt | Partial stick |
| Sprint | Left Shift | Left stick click |
| Jump / vault | Space | South (A / Cross) |
| Crouch | C | East (B / Circle) |
| Prone | Z | North (Y / Triangle) |

The test area has a low wall you can vault, a wall too tall to vault, and a crate too deep to vault.

## Layout

```
src/
  main.rs         app setup, test level, input, HUD
  camera.rs       camera
  locomotion/     state machine, sensors, sprint, traversal, tuning values
  weapon/         damage tables, melee, tuning values
reverse_engineering/
  VALUES.md       every number found so far, tagged confirmed / guess
  data/           decoded weapon damage table
  notes/          findings and working notes
  tools/          optional data-format helpers
```

Values in the code are either confirmed from game data or marked as placeholders to be measured in-game. See `reverse_engineering/README.md` for how each is tagged.

## Legal

*The Last of Us* is a trademark of Sony Interactive Entertainment LLC. This repository contains only original code and numeric findings. Raw game data and decompiled code are deliberately excluded, so do not commit them.
