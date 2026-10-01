use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{self, Clear, ClearType},
};
use rand;
use rodio::{self, Source, source::SineWave};
use std::{
    fs,
    io::{Write, stdout},
    sync::{Arc, Mutex},
    thread::{self},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const LOG_FILE: &str = "ch8.log";

struct Logger {
    log_file: fs::File,
}

impl Logger {
    pub fn log(&mut self, message: &str) {
        let now = SystemTime::now();
        let duration = now.duration_since(UNIX_EPOCH).unwrap();

        let millis = duration.as_millis();

        self.log_file
            .write(format!("[{}]", millis).as_bytes())
            .unwrap();
        self.log_file.write(message.as_bytes()).unwrap();
        self.log_file.write(b"\n").unwrap();
    }
}

struct Chip8 {
    // Memory architecture
    registers: [u8; 16],
    index: u16,
    stack: [u16; 32],
    stack_pointer: u8,
    delay_timer: u8,
    sound_timer: u8,

    frame_buffer: [[bool; 64]; 32],
    prev_frame_buffer: [[bool; 64]; 32],

    program_counter: u16,
    memory: [u8; 4096],

    keyboard: [bool; 16],

    waiting_for: Option<usize>,

    // Internals
    logger: Option<Logger>,
}

impl Chip8 {
    pub fn init_from(fp: &str) -> Chip8 {
        let program = fs::read(fp).expect("Failed to read program file");
        Chip8::init(&program)
    }

    pub fn init(program: &[u8]) -> Chip8 {
        if program.len() > (0xFFF - 0x200) {
            panic!("Program too long");
        }

        let mut program_array = [0u8; 4096];
        program_array[0x200..0x200 + program.len()].copy_from_slice(program);

        program_array[0x000..0x050].copy_from_slice(&[
            0xF0, 0x90, 0x90, 0x90, 0xF0, 0x20, 0x60, 0x20, 0x20, 0x70, 0xF0, 0x10, 0xF0, 0x80,
            0xF0, 0xF0, 0x10, 0xF0, 0x10, 0xF0, 0x90, 0x90, 0xF0, 0x10, 0x10, 0xF0, 0x80, 0xF0,
            0x10, 0xF0, 0xF0, 0x80, 0xF0, 0x90, 0xF0, 0xF0, 0x10, 0x20, 0x40, 0x40, 0xF0, 0x90,
            0xF0, 0x90, 0xF0, 0xF0, 0x90, 0xF0, 0x10, 0xF0, 0xF0, 0x90, 0xF0, 0x90, 0x90, 0xE0,
            0x90, 0xE0, 0x90, 0xE0, 0xF0, 0x80, 0x80, 0x80, 0xF0, 0xE0, 0x90, 0x90, 0x90, 0xE0,
            0xF0, 0x80, 0xF0, 0x80, 0xF0, 0xF0, 0x80, 0xF0, 0x80, 0x80,
        ]);

        Chip8 {
            registers: [0; 16],
            index: 0,
            stack: [0; 32],
            stack_pointer: 0,
            delay_timer: 0,
            sound_timer: 0,

            frame_buffer: [[false; 64]; 32],
            prev_frame_buffer: [[false; 64]; 32],

            program_counter: 0x200,
            memory: program_array,

            keyboard: [false; 16],

            waiting_for: None,

            // logger: None,
            logger: Some(Logger {
                log_file: fs::File::options().append(true).open(LOG_FILE).unwrap(),
            }),
        }
    }

    fn _ensure_bounds(x: usize, y: usize) {
        if x > 64 {
            panic!("Attempted to access pixel out of bounds (x = {})", x);
        }

        if y > 32 {
            panic!("Attempted to access pixel out of bounds (y = {})", y);
        }
    }

    fn get_opcode(&self) -> u16 {
        let bytes: [u8; 2] = self.memory
            [self.program_counter as usize..(self.program_counter + 2) as usize]
            .try_into()
            .unwrap();
        let opcode = u16::from_be_bytes(bytes);

        opcode
    }

    fn render(&self) {
        if self.frame_buffer == self.prev_frame_buffer {
            return;
        }

        let mut stdout = stdout();
        execute!(stdout, cursor::MoveTo(0, 0), Clear(ClearType::All)).unwrap();

        print!("┌");
        for _ in 0..24 {
            print!("─");
        }
        print!(" CHIP-8 Emulator ");
        for _ in 0..23 {
            print!("─");
        }
        print!("┐");
        execute!(stdout, cursor::MoveToNextLine(1)).unwrap();

        for y in 0..32 {
            print!("│");
            for x in 0..64 {
                let pixel = self.frame_buffer[y][x];
                if pixel {
                    print!("█");
                } else {
                    print!(" ");
                }
            }
            print!("│");
            execute!(stdout, cursor::MoveToNextLine(1)).unwrap();
        }
        print!("└");
        for _ in 0..64 {
            print!("─");
        }
        print!("┘");
        execute!(stdout, cursor::MoveToNextLine(1)).unwrap();
        stdout.flush().unwrap();
    }

    pub fn run(chip8: Arc<Mutex<Self>>) {
        let chip8_timer = Arc::clone(&chip8);
        let chip8_event = Arc::clone(&chip8);

        thread::spawn(move || {
            let tick = Duration::from_micros(16_667);

            let handle = rodio::DeviceSinkBuilder::open_default_sink().unwrap();
            let player = rodio::Player::connect_new(&handle.mixer());
            let source = SineWave::new(999.0)
                .take_duration(Duration::from_micros(16_667))
                .amplify(0.20);

            loop {
                {
                    let mut chip8 = chip8_timer.lock().unwrap();

                    if chip8.sound_timer > 0 {
                        chip8.sound_timer -= 1;
                        player.append(source.clone());
                        if let Some(logger) = &mut chip8.logger {
                            logger.log("[SOUND] Beep!");
                        }
                    }

                    if chip8.delay_timer > 0 {
                        chip8.delay_timer -= 1;
                    }
                }

                thread::sleep(tick);
            }
        });

        thread::spawn(move || {
            loop {
                if event::poll(Duration::from_millis(1)).unwrap() {
                    if let Event::Key(key_event) = event::read().unwrap() {
                        let mut chip8 = chip8_event.lock().unwrap();
                        let state = key_event.kind == KeyEventKind::Press;

                        let key: Option<usize> = match key_event.code {
                            KeyCode::Char('1') => Some(0x1),
                            KeyCode::Char('2') => Some(0x2),
                            KeyCode::Char('3') => Some(0x3),
                            KeyCode::Char('4') => Some(0x4),
                            KeyCode::Char('5') => Some(0x5),
                            KeyCode::Char('6') => Some(0x6),
                            KeyCode::Char('7') => Some(0x7),
                            KeyCode::Char('8') => Some(0x8),
                            KeyCode::Char('9') => Some(0x9),
                            KeyCode::Char('A') => Some(0xA),
                            KeyCode::Char('B') => Some(0xB),
                            KeyCode::Char('C') => Some(0xC),
                            KeyCode::Char('D') => Some(0xD),
                            KeyCode::Char('E') => Some(0xE),
                            KeyCode::Char('F') => Some(0xF),
                            _ => None,
                        };

                        if let Some(k) = key {
                            chip8.keyboard[k] = state;
                        }
                    }
                }
            }
        });

        loop {
            let mut chip8 = chip8.lock().unwrap();
            if let Some(x) = chip8.waiting_for {
                if let Some(key) = chip8.keyboard.iter().position(|&pressed| pressed) {
                    chip8.registers[x] = key as u8;
                    chip8.waiting_for = None;
                }

                continue;
            }

            let opcode = chip8.get_opcode();
            chip8.program_counter += 2;

            chip8.render();
            chip8.prev_frame_buffer = chip8.frame_buffer;

            if opcode == 0x00E0 {
                if let Some(logger) = &mut chip8.logger {
                    logger.log("[CLEAR] Clearing screen");
                }
                chip8.frame_buffer = [[false; 64]; 32];
            } else if opcode == 0x00EE {
                chip8.program_counter = chip8.stack[chip8.stack_pointer as usize];
                chip8.stack_pointer -= 1;
                if let Some(logger) = &mut chip8.logger {
                    logger.log("[RETURN] Returning from subroutine");
                }
            } else if 0x1000 <= opcode && opcode <= 0x1FFF {
                let nnn = opcode - 0x1000;
                chip8.program_counter = nnn;
                if let Some(logger) = &mut chip8.logger {
                    logger.log(format!("[JUMP] to {:#X}", nnn).as_str());
                }
            } else if 0x2000 <= opcode && opcode <= 0x2FFF {
                let nnn = opcode - 0x2000;
                chip8.stack_pointer += 1;
                let sp = chip8.stack_pointer;
                chip8.stack[sp as usize] = chip8.program_counter;
                chip8.program_counter = nnn;

                if let Some(logger) = &mut chip8.logger {
                    logger.log(format!("[CALL] to {:#X}", nnn).as_str());
                }
            } else if 0x3000 <= opcode && opcode <= 0x3FFF {
                let x = ((opcode >> 8) & 0x0F) as usize;
                let kk = (opcode & 0xFF) as u8;

                if chip8.registers[x] == kk {
                    chip8.program_counter += 2;
                }

                if let Some(logger) = &mut chip8.logger {
                    logger.log(format!("[SKIP] if V[{}] == {:#X}", x, kk).as_str());
                }
            } else if 0x4000 <= opcode && opcode <= 0x4FFF {
                let x = ((opcode >> 8) & 0x0F) as usize;
                let kk = (opcode & 0xFF) as u8;

                if chip8.registers[x] != kk {
                    chip8.program_counter += 2;
                }

                if let Some(logger) = &mut chip8.logger {
                    logger.log(format!("[SKIP] if V[{}] != {:#X}", x, kk).as_str());
                }
            } else if 0x5000 <= opcode && opcode <= 0x5FF0 {
                let x = ((opcode >> 8) & 0x0F) as usize;
                let y = ((opcode >> 4) & 0x0F) as usize;

                if chip8.registers[x] == chip8.registers[y] {
                    chip8.program_counter += 2;
                }

                if let Some(logger) = &mut chip8.logger {
                    logger.log(format!("[SKIP] if V[{}] == V[{}]", x, y).as_str());
                }
            } else if 0x6000 <= opcode && opcode <= 0x6FFF {
                let x = ((opcode >> 8) & 0x0F) as usize;
                let kk = (opcode & 0xFF) as u8;

                if let Some(logger) = &mut chip8.logger {
                    logger.log(format!("[SET] V[{}] = {:#X}", x, kk).as_str());
                }
                chip8.registers[x] = kk;
            } else if 0x7000 <= opcode && opcode <= 0x7FFF {
                let x = ((opcode >> 8) & 0x0F) as usize;
                let kk = (opcode & 0xFF) as u8;

                if let Some(logger) = &mut chip8.logger {
                    logger.log(format!("[ADD] V[{}] += {:#X}", x, kk).as_str());
                }

                chip8.registers[x] = chip8.registers[x].wrapping_add(kk);
            } else if 0x8000 <= opcode && opcode <= 0x8FFF {
                let digit = opcode & 0xF;

                let x = ((opcode >> 8) & 0x0F) as usize;
                let y = ((opcode >> 4) & 0x0F) as usize;

                if digit == 0 {
                    chip8.registers[x] = chip8.registers[y];
                    if let Some(logger) = &mut chip8.logger {
                        logger.log(format!("[SET] V[{}] = V[{}]", x, y).as_str());
                    }
                } else if digit == 1 {
                    chip8.registers[x] |= chip8.registers[y];
                    if let Some(logger) = &mut chip8.logger {
                        logger.log(format!("[OR] V[{}] |= V[{}]", x, y).as_str());
                    }
                } else if digit == 2 {
                    chip8.registers[x] &= chip8.registers[y];
                    if let Some(logger) = &mut chip8.logger {
                        logger.log(format!("[AND] V[{}] &= V[{}]", x, y).as_str());
                    }
                } else if digit == 3 {
                    chip8.registers[x] ^= chip8.registers[y];
                    if let Some(logger) = &mut chip8.logger {
                        logger.log(format!("[XOR] V[{}] ^= V[{}]", x, y).as_str());
                    }
                } else if digit == 4 {
                    let x_val = chip8.registers[x];
                    let y_val = chip8.registers[y];

                    let (res, overflow) = x_val.overflowing_add(y_val);
                    chip8.registers[x] = res;
                    chip8.registers[0xF] = overflow as u8;
                    if let Some(logger) = &mut chip8.logger {
                        logger.log(format!("[ADD] V[{}] += V[{}]", x, y).as_str());
                    }
                } else if digit == 5 {
                    let x_val = chip8.registers[x];
                    let y_val = chip8.registers[y];

                    chip8.registers[0xF] = (x_val >= y_val) as u8;
                    chip8.registers[x] = x_val.wrapping_sub(y_val);

                    if let Some(logger) = &mut chip8.logger {
                        logger.log(format!("[SUB] V[{}] -= V[{}]", x, y).as_str());
                    }
                } else if digit == 6 {
                    chip8.registers[0xF] = chip8.registers[x] & 0x1;
                    chip8.registers[x] >>= 1;
                    if let Some(logger) = &mut chip8.logger {
                        logger.log(format!("[SHR] V[{}] >>= 1", x).as_str());
                    }
                } else if digit == 7 {
                    let x_val = chip8.registers[x];
                    let y_val = chip8.registers[y];

                    chip8.registers[0xF] = (y_val >= x_val) as u8;
                    chip8.registers[x] = y_val.wrapping_sub(x_val);
                    if let Some(logger) = &mut chip8.logger {
                        logger.log(format!("[SUBN] V[{}] = V[{}] - V[{}]", x, y, x).as_str());
                    }
                } else if digit == 0xE {
                    chip8.registers[0xF] = chip8.registers[x] >> 7;
                    chip8.registers[x] <<= 1;
                    if let Some(logger) = &mut chip8.logger {
                        logger.log(format!("[SHL] V[{}] <<= 1", x).as_str());
                    }
                }
            } else if 0x9000 <= opcode && opcode <= 0x9FF0 {
                let x = ((opcode >> 8) & 0x0F) as usize;
                let y = ((opcode >> 4) & 0x0F) as usize;

                if chip8.registers[x] != chip8.registers[y] {
                    chip8.program_counter += 2;
                }

                if let Some(logger) = &mut chip8.logger {
                    logger.log(format!("[SKIP] if V[{}] != V[{}]", x, y).as_str());
                }
            } else if 0xA000 <= opcode && opcode <= 0xAFFF {
                let nnn = opcode - 0xA000;
                chip8.index = nnn;

                if let Some(logger) = &mut chip8.logger {
                    logger.log(format!("[SET] I = {:#X}", nnn).as_str());
                }
            } else if 0xB000 <= opcode && opcode <= 0xBFFF {
                let nnn = opcode - 0xB000;
                chip8.program_counter = (chip8.registers[0] as u16) + nnn;

                if let Some(logger) = &mut chip8.logger {
                    logger.log(format!("[JUMP] to V[0] + {:#X}", nnn).as_str());
                }
            } else if 0xC000 <= opcode && opcode <= 0xCFFF {
                let x = ((opcode >> 8) & 0x0F) as usize;
                let kk = (opcode & 0xFF) as u8;

                let r = rand::random_range(0..256) as u8;
                chip8.registers[x] = r & kk;

                if let Some(logger) = &mut chip8.logger {
                    logger.log(
                        format!("[RAND] V[{}] = rand() & {:#X} ({:#X})", x, kk, r & kk).as_str(),
                    );
                }
            } else if 0xD000 <= opcode && opcode <= 0xDFFF {
                let x = ((opcode >> 8) & 0x0F) as usize;
                let y = ((opcode >> 4) & 0x0F) as usize;
                let n = opcode & 0x0F;

                let vx = chip8.registers[x] as usize;
                let vy = chip8.registers[y] as usize;

                let data = chip8.memory[chip8.index as usize..(chip8.index + n) as usize].to_vec();

                if let Some(logger) = &mut chip8.logger {
                    logger.log(
                        format!(
                            "[DRAW] data = {:?}",
                            data.clone()
                                .iter()
                                .map(|x| format!("{:08b}", x))
                                .collect::<Vec<String>>()
                        )
                        .as_str(),
                    );
                }
                chip8.registers[0xF] = 0;
                for (y, byte) in data.iter().enumerate() {
                    let iy = (vy + y) % 32;
                    for x in (0..8).rev() {
                        let bit = ((byte >> x) & 1) == 1;
                        let ix = (vx + (7 - x)) % 64;

                        if chip8.frame_buffer[iy][ix] == bit && bit {
                            chip8.registers[0xF] = 1;
                        }

                        chip8.frame_buffer[iy][ix] ^= bit;
                    }
                }

                if let Some(logger) = &mut chip8.logger {
                    logger.log(
                        format!("[DRAW] V[{}] = {}, V[{}] = {}, N = {}", x, vx, y, vy, n).as_str(),
                    );
                }
            } else if 0xE000 <= opcode && opcode <= 0xEFFF {
                let selector = opcode & 0xFF;
                let x = ((opcode >> 8) & 0x0F) as usize;

                if selector == 0x9E {
                    if chip8.keyboard[x] {
                        chip8.program_counter += 2;
                    }

                    if let Some(logger) = &mut chip8.logger {
                        logger.log(format!("[SKIP] if key V[{}] is pressed", x).as_str());
                    }
                } else if selector == 0xA1 {
                    if !chip8.keyboard[x] {
                        chip8.program_counter += 2;
                    }

                    if let Some(logger) = &mut chip8.logger {
                        logger.log(format!("[SKIP] if key V[{}] is not pressed", x).as_str());
                    }
                }
            } else if 0xF000 <= opcode {
                let selector = opcode & 0xFF;
                let x = ((opcode >> 8) & 0x0F) as usize;

                if selector == 0x07 {
                    chip8.registers[x] = chip8.delay_timer;
                    if let Some(logger) = &mut chip8.logger {
                        logger.log(format!("[SET] V[{}] = delay_timer", x).as_str());
                    }
                } else if selector == 0x0A {
                    chip8.waiting_for = Some(x);
                    if let Some(logger) = &mut chip8.logger {
                        logger.log(format!("[WAIT] for key press to store in V[{}]", x).as_str());
                    }
                } else if selector == 0x15 {
                    chip8.delay_timer = chip8.registers[x];
                    if let Some(logger) = &mut chip8.logger {
                        logger.log(format!("[SET] delay_timer = V[{}]", x).as_str());
                    }
                } else if selector == 0x18 {
                    chip8.sound_timer = chip8.registers[x];
                    if let Some(logger) = &mut chip8.logger {
                        logger.log(format!("[SET] sound_timer = V[{}]", x).as_str());
                    }
                } else if selector == 0x1E {
                    chip8.index += chip8.registers[x] as u16;
                    if let Some(logger) = &mut chip8.logger {
                        logger.log(format!("[ADD] I += V[{}]", x).as_str());
                    }
                } else if selector == 0x29 {
                    chip8.index = (chip8.registers[x] * 5) as u16;
                    if let Some(logger) = &mut chip8.logger {
                        logger.log(format!("[SET] I = sprite for V[{}]", x).as_str());
                    }
                } else if selector == 0x33 {
                    let vx = chip8.registers[x];
                    let i = chip8.index as usize;
                    chip8.memory[i] = vx / 100;
                    chip8.memory[i + 1] = (vx / 10) % 10;
                    chip8.memory[i + 2] = vx % 10;
                    if let Some(logger) = &mut chip8.logger {
                        logger.log(format!("[STORE] BCD of V[{}] at I", x).as_str());
                    }
                } else if selector == 0x55 {
                    let i = chip8.index as usize;
                    for j in 0..x + 1 {
                        chip8.memory[i + j] = chip8.registers[j];
                    }

                    chip8.index += (x + 1) as u16;
                    if let Some(logger) = &mut chip8.logger {
                        logger.log(format!("[STORE] V[0..{}] at I", x).as_str());
                    }
                } else if selector == 0x65 {
                    let i = chip8.index as usize;
                    for j in 0..x + 1 {
                        chip8.registers[j] = chip8.memory[i + j];
                    }

                    chip8.index += (x + 1) as u16;
                    if let Some(logger) = &mut chip8.logger {
                        logger.log(format!("[LOAD] V[0..{}] from I", x).as_str());
                    }
                } else if selector == 0xFF {
                    // custom instruction!!
                    if let Some(logger) = &mut chip8.logger {
                        logger.log("[EXIT] Exiting emulator");
                    }
                    break;
                }
            }
        }
    }
}

fn main() {
    terminal::enable_raw_mode().unwrap();

    let args = std::env::args().collect::<Vec<String>>();
    if args.len() < 2 {
        println!("Usage: {} <program>", args[0]);
        return;
    }

    let program = &args[1];
    let chip = Arc::new(Mutex::new(Chip8::init_from(program)));

    Chip8::run(Arc::clone(&chip));

    terminal::disable_raw_mode().unwrap();
}
