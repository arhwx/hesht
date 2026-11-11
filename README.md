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

Quirk presets:

- `cosmac`: the original COSMAC VIP behaviour, for older ROMs
- `schip`: SUPER-CHIP behaviour, for newer ROMs

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
| Backspace | restart the ROM |
| Esc | quit |

## License

GPL-3.0-only. See [LICENSE](LICENSE).
