# tlou2-movement-bevy

A [Bevy](https://bevy.org) (Rust) reimplementation of *The Last of Us Part II*'s movement, camera and combat systems, built from reverse-engineered tuning data. Early prototype: placeholder capsule and boxes, no game assets.

> Unofficial fan project. Not affiliated with or endorsed by Naughty Dog or Sony Interactive Entertainment. Contains no original game code, models, audio or textures.

## Features

- **Locomotion**: state machine (walk, sprint with stamina, crouch, prone, dive, dodge, jump, vault/climb traversal, landings), sensors and a motion model.
- **Camera**: third-person orbit camera with aim and scope.
- **Weapons**: hunting rifle, pistol (with silencer), melee, ammo and reload, damage by weapon and target type.

## Run

Requires a recent stable Rust toolchain (edition 2024).

```
cargo run
```

The first build compiles Bevy and takes a while. Dependencies are optimized even in dev builds, so later runs are fast.

## Controls

| Action | Keyboard / mouse | Gamepad |
|---|---|---|
| Move | WASD | Left stick |
| Look | Mouse (click to lock, Esc to release) | Right stick |
| Walk (half speed) | Left Alt | Partial stick |
| Sprint | Left Shift | Left bumper |
| Jump / vault / climb | Space | South (A / Cross) |
| Crouch (tap) / prone (hold) | C | East (B / Circle) |
| Prone | Z | |
| Dodge | Q | Right bumper |
| Aim | Right mouse | Left trigger |
| Fire / melee (unaimed) | Left mouse | Right trigger |
| Reload | R | |
| Melee | F | West (X / Square) |
| Silencer | V | D-pad down |
| Weapon | 1 unarmed, 2 rifle, 3 pistol, 4 melee | D-pad up / left / right |
| Hold breath (scoped) | Shift | |
| Toggle sprint mode / infinite ammo | F1 / F2 | |

The test area has a low wall you can vault, a wall too tall to vault, a crate too deep to vault, and targets to shoot.

## Layout

```
src/
  main.rs         app setup, test level, input, HUD
  camera.rs       camera
  locomotion/     state machine, sensors, sprint, traversal, tuning values
  weapon/         damage tables, melee, tuning values
```

Tuning values live in each module's `tuning.rs`. Some are confirmed from game data and the rest are placeholders to be measured in-game.

## Legal

*The Last of Us* is a trademark of Sony Interactive Entertainment LLC. This repository contains only original code. Raw game data and decompiled code are deliberately excluded and must not be committed.
