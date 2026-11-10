use hesht::{Chip8, DISPLAY_HEIGHT, DISPLAY_WIDTH};
use minifb::{Key, Scale, Window, WindowOptions};
use rodio::Source;
use rodio::source::SquareWave;

const STEPS_PER_FRAME: usize = 10;
const ON: u32 = 0xFFFFFF;
const OFF: u32 = 0x000000;
const BEEP_HZ: f32 = 440.0;
const BEEP_VOLUME: f32 = 0.1;

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

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: hesht <rom>");
        std::process::exit(1);
    };

    if let Err(err) = run(&path) {
        eprintln!("hesht: {err}");
        std::process::exit(1);
    }
}

fn run(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let rom = std::fs::read(path).map_err(|err| format!("{path}: {err}"))?;
    let mut chip8 = Chip8::new();
    chip8.load_rom(&rom)?;

    let options = WindowOptions {
        scale: Scale::X16,
        ..WindowOptions::default()
    };
    let name = std::path::Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path);
    let title = format!("hesht - {name}");
    let mut window = Window::new(&title, DISPLAY_WIDTH, DISPLAY_HEIGHT, options)?;
    window.set_target_fps(60);

    let mut audio = rodio::DeviceSinkBuilder::open_default_sink()?;
    audio.log_on_drop(false);
    let beep = rodio::Player::connect_new(audio.mixer());
    beep.append(SquareWave::new(BEEP_HZ).amplify(BEEP_VOLUME));
    beep.pause();

    let mut buffer = [OFF; DISPLAY_WIDTH * DISPLAY_HEIGHT];
    while window.is_open() && !window.is_key_down(Key::Escape) {
        for (key, &code) in KEYS.iter().enumerate() {
            chip8.set_key(key as u8, window.is_key_down(code));
        }

        for _ in 0..STEPS_PER_FRAME {
            chip8.step()?;
        }
        chip8.tick_timers();
        if chip8.sound_active() {
            beep.play();
        } else {
            beep.pause();
        }

        for (pixel, &on) in buffer.iter_mut().zip(chip8.display()) {
            *pixel = if on { ON } else { OFF };
        }
        window.update_with_buffer(&buffer, DISPLAY_WIDTH, DISPLAY_HEIGHT)?;
    }

    Ok(())
}
