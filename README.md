# View Model for Horizon

[Русский](README.ru.md) · [Español](README.es.md)

Adjust the position, rotation, and scale of the models you see in first person. Main-hand and off-hand settings are independent and follow the logical hand when you change your dominant arm.

## Install and use

1. Copy [viewmodel.hmod](viewmodel.hmod) into your Horizon profile’s `horizon-mods` folder and enable **View Model for Horizon** under **Options → Mods**.
2. In a world, press **V** to open the editor. You can rebind **Open View Model** in **Options → Controls**. The same settings are available under **Options → Mods → View Model for Horizon → Config**.
3. Choose a hand and drag a slider. Scroll over a row or use the left and right arrow keys for small adjustments. Up, down, and Tab move between rows. Home resets the selected value. **Reset to default** resets that hand’s seven sliders.

Positive X moves right and positive Y moves up in view space. Depth (Z) moves the model toward you at positive values and away at negative values. Its −16 to +16 slider gradually approaches a bounded depth change so a large adjustment keeps the model on the same viewing ray. Scale changes the model’s size around its resting position and defaults to 1. Angles are in degrees. Settings are stored per Horizon profile.

| Transform target | What moves |
| --- | --- |
| Items and arms | The selected hand’s complete first-person geometry, including any map and its supporting arms |
| Items only | Held items rendered as ordinary item models |
| Arms only | The bare arm shown with an empty main hand |

The arm visibility toggle hides first-person arms, including the arms supporting maps. Held items and maps stay visible.

## Java parity

This release uses Horizon’s pre-pose hand transform, so positions can differ from Java when rotations, scaling, and item poses are combined. A held map and its supporting arms share one transform. Separate map and arm poses, swing controls, custom eat/drink animations, and equip animation overrides need additional Horizon renderer hooks.

## Build

Use Horizon’s `cargo horizon-mod` tool and Rust 1.99.1 with `rust-src`.

```sh
cargo horizon-mod build --wasm
```

The SDK is pinned in `Cargo.toml`. The build writes `target/horizon-mod/viewmodel.hmod` and updates the WASM digest in the manifest.

## Credits

Original View Model for Minecraft Java by **I-No-oNe**. This port follows the [26.3.x Java source](https://github.com/I-No-oNe/View-Model/tree/d3ea13d5432446be42b60da35f3f4a58865f65ee) and uses the original icon. Distributed under the [MIT license](LICENSE).
