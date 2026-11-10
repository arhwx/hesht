pub const MEMORY_SIZE: usize = 4096;
pub const PROGRAM_START: u16 = 0x200;
pub const FONT_START: usize = 0x050;
pub const DISPLAY_WIDTH: usize = 64;
pub const DISPLAY_HEIGHT: usize = 32;

const FONT: [u8; 80] = [
    0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
    0x20, 0x60, 0x20, 0x20, 0x70, // 1
    0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
    0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
    0x90, 0x90, 0xF0, 0x10, 0x10, // 4
    0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
    0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
    0xF0, 0x10, 0x20, 0x40, 0x40, // 7
    0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
    0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
    0xF0, 0x90, 0xF0, 0x90, 0x90, // A
    0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
    0xF0, 0x80, 0x80, 0x80, 0xF0, // C
    0xE0, 0x90, 0x90, 0x90, 0xE0, // D
    0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
    0xF0, 0x80, 0xF0, 0x80, 0x80, // F
];

#[derive(Debug, PartialEq, Eq)]
pub enum LoadError {
    TooLarge { size: usize, max: usize },
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge { size, max } => {
                write!(
                    f,
                    "rom is {size} bytes, but at most {max} bytes fit in memory"
                )
            }
        }
    }
}

impl std::error::Error for LoadError {}

#[derive(Debug, PartialEq, Eq)]
pub enum StepError {
    UnknownOpcode { opcode: u16, addr: u16 },
    StackOverflow { addr: u16 },
    StackUnderflow { addr: u16 },
}

impl std::fmt::Display for StepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownOpcode { opcode, addr } => {
                write!(f, "unknown opcode {opcode:04X} at {addr:03X}")
            }
            Self::StackOverflow { addr } => write!(f, "stack overflow at {addr:03X}"),
            Self::StackUnderflow { addr } => write!(f, "stack underflow at {addr:03X}"),
        }
    }
}

impl std::error::Error for StepError {}

/// behaviours that differ between CHIP-8 interpreters
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quirks {
    /// 8xy1, 8xy2 and 8xy3 reset VF to 0
    pub vf_reset: bool,
    /// 8xy6 and 8xyE shift vY into vX instead of shifting vX in place
    pub shift_uses_vy: bool,
    /// Fx55 and Fx65 leave I pointing past the last register
    pub memory_increments_i: bool,
    /// Bxnn jumps to xnn plus vX instead of nnn plus V0
    pub jump_uses_vx: bool,
    /// Dxyn stops execution until the next 60 Hz tick
    pub display_wait: bool,
}

impl Quirks {
    pub const COSMAC: Self = Self {
        vf_reset: true,
        shift_uses_vy: true,
        memory_increments_i: true,
        jump_uses_vx: false,
        display_wait: true,
    };

    pub const SCHIP: Self = Self {
        vf_reset: false,
        shift_uses_vy: false,
        memory_increments_i: false,
        jump_uses_vx: true,
        display_wait: false,
    };
}

impl Default for Quirks {
    fn default() -> Self {
        Self::COSMAC
    }
}

pub struct Chip8 {
    memory: [u8; MEMORY_SIZE],
    v: [u8; 16],
    i: u16,
    pc: u16,
    stack: [u16; 16],
    sp: u8,
    delay_timer: u8,
    sound_timer: u8,
    display: [bool; DISPLAY_WIDTH * DISPLAY_HEIGHT],
    keys: [bool; 16],
    pressed_key: Option<u8>,
    rng: u32,
    quirks: Quirks,
    waiting_for_tick: bool,
}

impl Chip8 {
    pub fn new() -> Self {
        Self::with_quirks(Quirks::default())
    }

    pub fn with_quirks(quirks: Quirks) -> Self {
        let mut memory = [0; MEMORY_SIZE];
        memory[FONT_START..FONT_START + FONT.len()].copy_from_slice(&FONT);
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos());

        Self {
            memory,
            v: [0; 16],
            i: 0,
            pc: PROGRAM_START,
            stack: [0; 16],
            sp: 0,
            delay_timer: 0,
            sound_timer: 0,
            display: [false; DISPLAY_WIDTH * DISPLAY_HEIGHT],
            keys: [false; 16],
            pressed_key: None,
            rng: seed | 1,
            quirks,
            waiting_for_tick: false,
        }
    }

    pub fn load_rom(&mut self, rom: &[u8]) -> Result<(), LoadError> {
        let program = &mut self.memory[PROGRAM_START as usize..];
        if rom.len() > program.len() {
            return Err(LoadError::TooLarge {
                size: rom.len(),
                max: program.len(),
            });
        }

        program.fill(0);
        program[..rom.len()].copy_from_slice(rom);
        Ok(())
    }

    pub fn tick(&mut self) {
        self.delay_timer = self.delay_timer.saturating_sub(1);
        self.sound_timer = self.sound_timer.saturating_sub(1);
        self.waiting_for_tick = false;
    }

    pub fn sound_active(&self) -> bool {
        self.sound_timer > 0
    }

    pub fn display(&self) -> &[bool] {
        &self.display
    }

    pub fn set_key(&mut self, key: u8, pressed: bool) {
        self.keys[key as usize] = pressed;
    }

    pub fn step(&mut self) -> Result<(), StepError> {
        if self.waiting_for_tick {
            return Ok(());
        }

        let addr = self.pc;
        let opcode = self.fetch();
        self.execute(opcode, addr)
    }

    fn fetch(&mut self) -> u16 {
        let pc = self.pc as usize;
        let opcode = u16::from_be_bytes([self.memory[pc], self.memory[(pc + 1) % MEMORY_SIZE]]);
        self.pc = (self.pc + 2) % MEMORY_SIZE as u16;
        opcode
    }

    fn execute(&mut self, opcode: u16, addr: u16) -> Result<(), StepError> {
        let x = ((opcode >> 8) & 0xF) as usize;
        let y = ((opcode >> 4) & 0xF) as usize;
        let n = (opcode & 0xF) as u8;
        let nn = (opcode & 0xFF) as u8;
        let nnn = opcode & 0xFFF;

        match opcode >> 12 {
            0x0 if opcode == 0x00E0 => self.display.fill(false),
            0x0 if opcode == 0x00EE => {
                if self.sp == 0 {
                    return Err(StepError::StackUnderflow { addr });
                }
                self.sp -= 1;
                self.pc = self.stack[self.sp as usize];
            }
            0x1 => self.pc = nnn,
            0x2 => {
                if self.sp as usize == self.stack.len() {
                    return Err(StepError::StackOverflow { addr });
                }
                self.stack[self.sp as usize] = self.pc;
                self.sp += 1;
                self.pc = nnn;
            }
            0x3 => self.skip_if(self.v[x] == nn),
            0x4 => self.skip_if(self.v[x] != nn),
            0x5 if n == 0 => self.skip_if(self.v[x] == self.v[y]),
            0x6 => self.v[x] = nn,
            0x7 => self.v[x] = self.v[x].wrapping_add(nn),
            0x8 => match n {
                0x0 => self.v[x] = self.v[y],
                0x1 => self.logic(x, self.v[x] | self.v[y]),
                0x2 => self.logic(x, self.v[x] & self.v[y]),
                0x3 => self.logic(x, self.v[x] ^ self.v[y]),
                0x4 => {
                    let (sum, carry) = self.v[x].overflowing_add(self.v[y]);
                    self.v[x] = sum;
                    self.v[0xF] = carry as u8;
                }
                0x5 => {
                    let (diff, borrow) = self.v[x].overflowing_sub(self.v[y]);
                    self.v[x] = diff;
                    self.v[0xF] = !borrow as u8;
                }
                0x6 => {
                    let value = self.shift_operand(x, y);
                    self.v[x] = value >> 1;
                    self.v[0xF] = value & 1;
                }
                0x7 => {
                    let (diff, borrow) = self.v[y].overflowing_sub(self.v[x]);
                    self.v[x] = diff;
                    self.v[0xF] = !borrow as u8;
                }
                0xE => {
                    let value = self.shift_operand(x, y);
                    self.v[x] = value << 1;
                    self.v[0xF] = value >> 7;
                }
                _ => return Err(StepError::UnknownOpcode { opcode, addr }),
            },
            0x9 if n == 0 => self.skip_if(self.v[x] != self.v[y]),
            0xA => self.i = nnn,
            0xB => {
                let offset = if self.quirks.jump_uses_vx {
                    self.v[x]
                } else {
                    self.v[0]
                };
                self.pc = (nnn + offset as u16) % MEMORY_SIZE as u16;
            }
            0xC => self.v[x] = self.next_random() & nn,
            0xD => {
                self.draw(self.v[x], self.v[y], n);
                self.waiting_for_tick = self.quirks.display_wait;
            }
            0xE => match nn {
                0x9E => self.skip_if(self.keys[(self.v[x] & 0xF) as usize]),
                0xA1 => self.skip_if(!self.keys[(self.v[x] & 0xF) as usize]),
                _ => return Err(StepError::UnknownOpcode { opcode, addr }),
            },
            0xF => match nn {
                0x07 => self.v[x] = self.delay_timer,
                0x0A => match self.pressed_key {
                    Some(key) if !self.keys[key as usize] => {
                        self.v[x] = key;
                        self.pressed_key = None;
                    }
                    Some(_) => self.pc = addr,
                    None => {
                        self.pressed_key = self
                            .keys
                            .iter()
                            .position(|&pressed| pressed)
                            .map(|key| key as u8);
                        self.pc = addr;
                    }
                },
                0x15 => self.delay_timer = self.v[x],
                0x18 => self.sound_timer = self.v[x],
                0x1E => self.i = (self.i + self.v[x] as u16) % MEMORY_SIZE as u16,
                0x29 => self.i = (FONT_START + (self.v[x] & 0xF) as usize * 5) as u16,
                0x33 => {
                    let i = self.i as usize;
                    let value = self.v[x];
                    self.memory[i % MEMORY_SIZE] = value / 100;
                    self.memory[(i + 1) % MEMORY_SIZE] = value / 10 % 10;
                    self.memory[(i + 2) % MEMORY_SIZE] = value % 10;
                }
                0x55 => {
                    let i = self.i as usize;
                    for (offset, &value) in self.v[..=x].iter().enumerate() {
                        self.memory[(i + offset) % MEMORY_SIZE] = value;
                    }
                    if self.quirks.memory_increments_i {
                        self.i = ((i + x + 1) % MEMORY_SIZE) as u16;
                    }
                }
                0x65 => {
                    let i = self.i as usize;
                    for (offset, value) in self.v[..=x].iter_mut().enumerate() {
                        *value = self.memory[(i + offset) % MEMORY_SIZE];
                    }
                    if self.quirks.memory_increments_i {
                        self.i = ((i + x + 1) % MEMORY_SIZE) as u16;
                    }
                }
                _ => return Err(StepError::UnknownOpcode { opcode, addr }),
            },
            _ => return Err(StepError::UnknownOpcode { opcode, addr }),
        }
        Ok(())
    }

    fn skip_if(&mut self, condition: bool) {
        if condition {
            self.pc = (self.pc + 2) % MEMORY_SIZE as u16;
        }
    }

    fn logic(&mut self, x: usize, value: u8) {
        self.v[x] = value;
        if self.quirks.vf_reset {
            self.v[0xF] = 0;
        }
    }

    fn shift_operand(&self, x: usize, y: usize) -> u8 {
        if self.quirks.shift_uses_vy {
            self.v[y]
        } else {
            self.v[x]
        }
    }

    fn next_random(&mut self) -> u8 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng >> 24) as u8
    }

    fn draw(&mut self, x: u8, y: u8, height: u8) {
        let x = x as usize % DISPLAY_WIDTH;
        let y = y as usize % DISPLAY_HEIGHT;
        self.v[0xF] = 0;

        for row in 0..(height as usize).min(DISPLAY_HEIGHT - y) {
            let byte = self.memory[(self.i as usize + row) % MEMORY_SIZE];
            for col in 0..(DISPLAY_WIDTH - x).min(8) {
                if byte & (0x80 >> col) == 0 {
                    continue;
                }

                let pixel = &mut self.display[(y + row) * DISPLAY_WIDTH + x + col];
                if *pixel {
                    self.v[0xF] = 1;
                }
                *pixel = !*pixel;
            }
        }
    }
}

impl Default for Chip8 {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chip8(program: &[u16]) -> Chip8 {
        chip8_with(Quirks::default(), program)
    }

    fn chip8_with(quirks: Quirks, program: &[u16]) -> Chip8 {
        let rom: Vec<u8> = program.iter().flat_map(|op| op.to_be_bytes()).collect();
        let mut chip8 = Chip8::with_quirks(quirks);
        chip8.load_rom(&rom).unwrap();
        chip8
    }

    fn run(chip8: &mut Chip8, steps: usize) {
        for _ in 0..steps {
            chip8.step().unwrap();
        }
    }

    fn cosmac_and_schip(program: &[u16], steps: usize) -> (Chip8, Chip8) {
        let mut cosmac = chip8_with(Quirks::COSMAC, program);
        let mut schip = chip8_with(Quirks::SCHIP, program);
        run(&mut cosmac, steps);
        run(&mut schip, steps);
        (cosmac, schip)
    }

    #[test]
    fn rom_too_large() {
        let mut chip8 = Chip8::new();
        assert_eq!(chip8.load_rom(&[0; 3584]), Ok(()));
        assert_eq!(
            chip8.load_rom(&[0; 3585]),
            Err(LoadError::TooLarge {
                size: 3585,
                max: 3584
            })
        );
    }

    #[test]
    fn call_and_return() {
        let mut chip8 = chip8(&[0x2204, 0x1202, 0x6007, 0x00EE]);
        run(&mut chip8, 3);
        assert_eq!(chip8.pc, 0x202);
        assert_eq!(chip8.sp, 0);
        assert_eq!(chip8.v[0], 7);
    }

    #[test]
    fn stack_overflow() {
        let mut chip8 = chip8(&[0x2200]);
        run(&mut chip8, 16);
        assert_eq!(chip8.step(), Err(StepError::StackOverflow { addr: 0x200 }));
    }

    #[test]
    fn stack_underflow() {
        let mut chip8 = chip8(&[0x00EE]);
        assert_eq!(chip8.step(), Err(StepError::StackUnderflow { addr: 0x200 }));
    }

    #[test]
    fn unknown_opcode() {
        let mut chip8 = chip8(&[0x6000, 0xFFFF]);
        run(&mut chip8, 1);
        assert_eq!(
            chip8.step(),
            Err(StepError::UnknownOpcode {
                opcode: 0xFFFF,
                addr: 0x202
            })
        );
    }

    #[test]
    fn add_carry() {
        let mut chip8 = chip8(&[0x60FF, 0x6102, 0x8014]);
        run(&mut chip8, 3);
        assert_eq!(chip8.v[0], 1);
        assert_eq!(chip8.v[0xF], 1);
    }

    #[test]
    fn sub_borrow() {
        let mut chip8 = chip8(&[0x6003, 0x6105, 0x8015, 0x6203, 0x8217]);
        run(&mut chip8, 3);
        assert_eq!(chip8.v[0], 0xFE);
        assert_eq!(chip8.v[0xF], 0);
        run(&mut chip8, 2);
        assert_eq!(chip8.v[2], 2);
        assert_eq!(chip8.v[0xF], 1);
    }

    #[test]
    fn flag_overwrites_result() {
        let mut chip8 = chip8(&[0x6F80, 0x6180, 0x8F14, 0x6F05, 0x6103, 0x8F15]);
        run(&mut chip8, 3);
        assert_eq!(chip8.v[0xF], 1);
        run(&mut chip8, 3);
        assert_eq!(chip8.v[0xF], 1);
    }

    #[test]
    fn bcd() {
        let mut chip8 = chip8(&[0x60FE, 0xA300, 0xF033]);
        run(&mut chip8, 3);
        assert_eq!(chip8.memory[0x300..0x303], [2, 5, 4]);
    }

    #[test]
    fn wait_for_key_release() {
        let mut chip8 = chip8(&[0xF00A]);
        run(&mut chip8, 1);
        assert_eq!(chip8.pc, 0x200);
        chip8.set_key(5, true);
        run(&mut chip8, 1);
        assert_eq!(chip8.pc, 0x200);
        chip8.set_key(5, false);
        run(&mut chip8, 1);
        assert_eq!(chip8.pc, 0x202);
        assert_eq!(chip8.v[0], 5);
    }

    #[test]
    fn draw_collision() {
        let mut chip8 = chip8(&[0xA050, 0x6000, 0xD005, 0xD005]);
        run(&mut chip8, 3);
        assert_eq!(chip8.display[..5], [true, true, true, true, false]);
        assert_eq!(chip8.v[0xF], 0);
        chip8.tick();
        run(&mut chip8, 1);
        assert!(chip8.display.iter().all(|&on| !on));
        assert_eq!(chip8.v[0xF], 1);
    }

    #[test]
    fn draw_clipping() {
        let mut chip8 = chip8(&[0xA050, 0x603E, 0x611E, 0xD015]);
        run(&mut chip8, 4);
        let on: Vec<usize> = (0..chip8.display.len())
            .filter(|&i| chip8.display[i])
            .collect();
        assert_eq!(on, [30 * 64 + 62, 30 * 64 + 63, 31 * 64 + 62]);
    }

    #[test]
    fn vf_reset() {
        let (cosmac, schip) = cosmac_and_schip(&[0x6F05, 0x8011], 2);
        assert_eq!(cosmac.v[0xF], 0);
        assert_eq!(schip.v[0xF], 5);
    }

    #[test]
    fn shift() {
        let (cosmac, schip) = cosmac_and_schip(&[0x6006, 0x6103, 0x8016], 3);
        assert_eq!((cosmac.v[0], cosmac.v[0xF]), (1, 1));
        assert_eq!((schip.v[0], schip.v[0xF]), (3, 0));
    }

    #[test]
    fn memory_increment() {
        let (cosmac, schip) = cosmac_and_schip(&[0xA300, 0xF255], 2);
        assert_eq!(cosmac.i, 0x303);
        assert_eq!(schip.i, 0x300);
    }

    #[test]
    fn jump() {
        let (cosmac, schip) = cosmac_and_schip(&[0x6002, 0x6104, 0xB140], 3);
        assert_eq!(cosmac.pc, 0x142);
        assert_eq!(schip.pc, 0x144);
    }

    #[test]
    fn display_wait() {
        let (mut cosmac, schip) = cosmac_and_schip(&[0xD001, 0x6005], 2);
        assert_eq!(cosmac.v[0], 0);
        assert_eq!(schip.v[0], 5);
        cosmac.tick();
        run(&mut cosmac, 1);
        assert_eq!(cosmac.v[0], 5);
    }
}
