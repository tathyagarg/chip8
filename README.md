![Demo GIF](.github/assets/demo.gif)

# Chip-8 Emulator

A little chip-8 emulator written in Rust. Made as an exercise to understand how emulators work. It uses terminal based graphics to display the output of the emulator (since the CHIP-8 has a 64x32 monochrome display). The emulator is capable of running most CHIP-8 programs.

Built based on Columbia University's Embedded Systems 4840 Course's CHIP-8 Design Specification (found [here](https://www.cs.columbia.edu/~sedwards/classes/2016/4840-spring/designs/Chip8.pdf))

This is a really small project and extremely simple to implement so I was able to get through it in a few hours. No AI was used in the making of this project at all (I specifically disabled Copilot for this project).

## Features

- Supports all CHIP-8 opcodes
- Terminal based graphics (using the `crossterm` crate)
- Keyboard input (using the `crossterm` crate)
- Sound (using the `rodio` crate)
- Built-in ROMs for testing (nothing fancy)
- (Hopefully) cross-platform

## Installation

### Pre-built Binaries

Download the latest release from the [releases page](https://github.com/tathyagarg/chip8/releases) and run the binary. You can also build from source if you want to.

### Building from Source

Step 1: Clone the repository

```bash
git clone https://github.com/tathyagarg/chip8.git
cd chip8
```

Step 2: Build the project

```bash
cargo build --release
```

Step 3: Run the emulator

```bash
cargo run --release -- -p <path_to_chip8_rom>
```

## Usage

The emulator can be run from the command line. The following options are available:

- `-p <path_to_chip8_rom>`: Path to the CHIP-8 ROM file to load and run.
- `-c`: If this flag is set, the program file is treated as a text file containing CHIP-8 opcodes (not instructions), like [this](programs/test.txt). This is useful for testing the emulator with custom programs.
- `--log-file`: The file to write logs to. If not specified, no logs will be written. Logs are useful for debugging.
- `-h`: Display help information.
