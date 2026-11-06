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
}

impl Chip8 {
    pub fn new() -> Self {
        let mut memory = [0; MEMORY_SIZE];
        memory[FONT_START..FONT_START + FONT.len()].copy_from_slice(&FONT);

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

    pub fn tick_timers(&mut self) {
        self.delay_timer = self.delay_timer.saturating_sub(1);
        self.sound_timer = self.sound_timer.saturating_sub(1);
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
            0x9 if n == 0 => self.skip_if(self.v[x] != self.v[y]),
            0xA => self.i = nnn,
            0xB => self.pc = (nnn + self.v[0] as u16) % MEMORY_SIZE as u16,
            _ => return Err(StepError::UnknownOpcode { opcode, addr }),
        }
        Ok(())
    }

    fn skip_if(&mut self, condition: bool) {
        if condition {
            self.pc = (self.pc + 2) % MEMORY_SIZE as u16;
        }
    }
}

impl Default for Chip8 {
    fn default() -> Self {
        Self::new()
    }
}
