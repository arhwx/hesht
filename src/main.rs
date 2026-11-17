use hesht::{Chip8, DISPLAY_HEIGHT, DISPLAY_WIDTH, LoadError, Quirks};
use minifb::{Key, KeyRepeat, Scale, Window, WindowOptions};
use rodio::Source;
use rodio::source::SquareWave;

const DEFAULT_SPEED: usize = 10;
const DEFAULT_FG: u32 = 0xFFFFFF;
const DEFAULT_BG: u32 = 0x000000;
const BEEP_HZ: f32 = 440.0;
const BEEP_VOLUME: f32 = 0.1;

const USAGE: &str = "usage: hesht [options] <rom>

options:
  --speed N        instructions per frame (default 10)
  --scale N        window scale: 1, 2, 4, 8, 16 or 32 (default 16)
  --quirks NAME    cosmac or schip (default cosmac)
  --quirk NAME=on|off
                   override one quirk of the preset, can be repeated
  --fg RRGGBB      colour of lit pixels (default FFFFFF)
  --bg RRGGBB      colour of unlit pixels (default 000000)

quirks:
  vf-reset, shift-uses-vy, memory-increments-i, jump-uses-vx,
  display-wait, clipping";

const KEYS: [Key; 16] = [
    Key::X,
    Key::Key1,
    Key::Key2,
    Key::Key3,
    Key::Q,
    Key::W,
    Key::E,
    Key::A,
    Key::S,
    Key::D,
    Key::Z,
    Key::C,
    Key::Key4,
    Key::R,
    Key::F,
    Key::V,
];

struct Options {
    rom: String,
    speed: usize,
    scale: Scale,
    quirks: Quirks,
    fg: u32,
    bg: u32,
}

fn main() {
    let options = match parse_args(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(err) => {
            eprintln!("hesht: {err}\n\n{USAGE}");
            std::process::exit(1);
        }
    };

    if let Err(err) = run(&options) {
        eprintln!("hesht: {err}");
        std::process::exit(1);
    }
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut rom = None;
    let mut speed = DEFAULT_SPEED;
    let mut scale = Scale::X16;
    let mut quirks = Quirks::default();
    let mut overrides = Vec::new();
    let mut fg = DEFAULT_FG;
    let mut bg = DEFAULT_BG;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            "--speed" => speed = parse_speed(&value(&mut args, &arg)?)?,
            "--scale" => scale = parse_scale(&value(&mut args, &arg)?)?,
            "--quirks" => quirks = parse_quirks(&value(&mut args, &arg)?)?,
            "--quirk" => overrides.push(parse_quirk(&value(&mut args, &arg)?)?),
            "--fg" => fg = parse_colour(&value(&mut args, &arg)?)?,
            "--bg" => bg = parse_colour(&value(&mut args, &arg)?)?,
            _ if arg.starts_with('-') => return Err(format!("unknown option {arg}")),
            _ if rom.is_none() => rom = Some(arg),
            _ => return Err(format!("unexpected argument {arg}")),
        }
    }

    for (name, enabled) in overrides {
        let quirk = quirks
            .get_mut(&name)
            .ok_or_else(|| format!("unknown quirk {name}"))?;
        *quirk = enabled;
    }

    Ok(Options {
        rom: rom.ok_or("missing rom")?,
        speed,
        scale,
        quirks,
        fg,
        bg,
    })
}

fn value(args: &mut impl Iterator<Item = String>, option: &str) -> Result<String, String> {
    args.next().ok_or_else(|| format!("{option} needs a value"))
}

fn parse_speed(value: &str) -> Result<usize, String> {
    match value.parse::<usize>() {
        Ok(speed) if speed > 0 => Ok(speed),
        _ => Err(format!("invalid speed {value}, expected a positive number")),
    }
}

fn parse_scale(value: &str) -> Result<Scale, String> {
    match value {
        "1" => Ok(Scale::X1),
        "2" => Ok(Scale::X2),
        "4" => Ok(Scale::X4),
        "8" => Ok(Scale::X8),
        "16" => Ok(Scale::X16),
        "32" => Ok(Scale::X32),
        _ => Err(format!(
            "invalid scale {value}, expected 1, 2, 4, 8, 16 or 32"
        )),
    }
}

fn parse_quirks(value: &str) -> Result<Quirks, String> {
    match value {
        "cosmac" => Ok(Quirks::COSMAC),
        "schip" => Ok(Quirks::SCHIP),
        _ => Err(format!("invalid quirks {value}, expected cosmac or schip")),
    }
}

fn parse_quirk(value: &str) -> Result<(String, bool), String> {
    let invalid = || format!("invalid quirk {value}, expected NAME=on or NAME=off");
    let (name, state) = value.split_once('=').ok_or_else(invalid)?;
    let enabled = match state {
        "on" => true,
        "off" => false,
        _ => return Err(invalid()),
    };
    Ok((name.to_string(), enabled))
}

fn parse_colour(value: &str) -> Result<u32, String> {
    let hex = value.strip_prefix('#').unwrap_or(value);
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("invalid colour {value}, expected RRGGBB"));
    }
    Ok(u32::from_str_radix(hex, 16).expect("six hex digits fit in u32"))
}

fn run(options: &Options) -> Result<(), Box<dyn std::error::Error>> {
    let path = &options.rom;
    let rom = std::fs::read(path).map_err(|err| format!("{path}: {err}"))?;
    let mut chip8 = boot(&rom, options.quirks)?;

    let window_options = WindowOptions {
        scale: options.scale,
        ..WindowOptions::default()
    };
    let name = std::path::Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path);
    let mut speed = options.speed;
    let mut paused = false;
    let mut window = Window::new(
        &title(name, speed, paused),
        DISPLAY_WIDTH,
        DISPLAY_HEIGHT,
        window_options,
    )?;
    window.set_target_fps(60);

    let mut audio = rodio::DeviceSinkBuilder::open_default_sink()?;
    audio.log_on_drop(false);
    let beep = rodio::Player::connect_new(audio.mixer());
    beep.append(SquareWave::new(BEEP_HZ).amplify(BEEP_VOLUME));
    beep.pause();

    let mut buffer = [options.bg; DISPLAY_WIDTH * DISPLAY_HEIGHT];
    while window.is_open() && !window.is_key_down(Key::Escape) {
        let mut retitle = false;
        if window.is_key_pressed(Key::Space, KeyRepeat::No) {
            paused = !paused;
            retitle = true;
        }
        if window.is_key_pressed(Key::Equal, KeyRepeat::Yes) {
            speed = speed.saturating_add(1);
            retitle = true;
        }
        if window.is_key_pressed(Key::Minus, KeyRepeat::Yes) && speed > 1 {
            speed -= 1;
            retitle = true;
        }
        if retitle {
            window.set_title(&title(name, speed, paused));
        }
        if window.is_key_pressed(Key::Backspace, KeyRepeat::No) {
            chip8 = boot(&rom, options.quirks)?;
        }

        let step_frame = paused && window.is_key_pressed(Key::N, KeyRepeat::Yes);
        if !paused || step_frame {
            for (key, &code) in KEYS.iter().enumerate() {
                chip8.set_key(key as u8, window.is_key_down(code));
            }

            for _ in 0..speed {
                chip8.step()?;
            }
            chip8.tick();
        }
        if !paused && chip8.sound_active() {
            beep.play();
        } else {
            beep.pause();
        }

        for (pixel, &on) in buffer.iter_mut().zip(chip8.display()) {
            *pixel = if on { options.fg } else { options.bg };
        }
        window.update_with_buffer(&buffer, DISPLAY_WIDTH, DISPLAY_HEIGHT)?;
    }

    Ok(())
}

fn title(name: &str, speed: usize, paused: bool) -> String {
    let state = if paused { " (paused)" } else { "" };
    format!("hesht - {name} - speed {speed}{state}")
}

fn boot(rom: &[u8], quirks: Quirks) -> Result<Chip8, LoadError> {
    let mut chip8 = Chip8::with_quirks(quirks);
    chip8.load_rom(rom)?;
    Ok(chip8)
}
