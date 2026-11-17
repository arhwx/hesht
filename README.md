<p align="center"><img src="assets/banner.svg" alt="hesht"></p>

<p align="center">A CHIP-8 emulator written in Rust, named after the Kurdish word for eight.</p>

## Building

You need Rust 1.87 or newer. On Linux, sound goes through ALSA, so you also need `pkg-config` and the ALSA development files: `libasound2-dev` on Debian and Ubuntu, `alsa-lib-devel` on Fedora, `alsa-lib` on Arch.

```sh
cargo build --release
```

## Usage

```sh
hesht [options] <rom>
```

| Option | Meaning | Default |
|---|---|---|
| `--speed N` | instructions per frame | 10 |
| `--scale N` | window scale: 1, 2, 4, 8, 16 or 32 | 16 |
| `--quirks NAME` | `cosmac` or `schip` | `cosmac` |
| `--quirk NAME=on\|off` | override one quirk of the preset, can be repeated | |
| `--fg RRGGBB` | colour of lit pixels | `FFFFFF` |
| `--bg RRGGBB` | colour of unlit pixels | `000000` |

Quirk presets:

- `cosmac`: the original COSMAC VIP behaviour, for older ROMs
- `schip`: SUPER-CHIP behaviour, for newer ROMs

Individual quirks, and whether each preset turns them on:

| Quirk | Behaviour when on | `cosmac` | `schip` |
|---|---|---|---|
| `vf-reset` | `8xy1`, `8xy2` and `8xy3` reset VF to 0 | on | off |
| `shift-uses-vy` | `8xy6` and `8xyE` shift vY into vX | on | off |
| `memory-increments-i` | `Fx55` and `Fx65` leave I past the last register | on | off |
| `jump-uses-vx` | `Bxnn` jumps to xnn plus vX instead of nnn plus V0 | off | on |
| `display-wait` | `Dxyn` waits for the next 60 Hz tick | on | off |
| `clipping` | sprites are clipped at the screen edge instead of wrapping | on | on |

Overrides apply on top of the preset, wherever they appear on the command line:

```sh
hesht --quirks schip --quirk clipping=off game.ch8
```

The emulator runs at 60 frames per second, so the default speed is 600 instructions per second.

## Controls

The CHIP-8 keypad maps to the left side of the keyboard:

```
keypad     keyboard
1 2 3 C    1 2 3 4
4 5 6 D    Q W E R
7 8 9 E    A S D F
A 0 B F    Z X C V
```

| Key | Action |
|---|---|
| Space | pause and resume |
| N | while paused, run one frame |
| `-` and `=` | lower and raise the speed by one instruction per frame |
| Backspace | restart the ROM |
| Esc | quit |

## License

GPL-3.0-only. See [LICENSE](LICENSE).
