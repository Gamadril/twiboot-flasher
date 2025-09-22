# TWI Bootloader Flasher

Rust implementation of the [TWI/I2C bootloader](https://github.com/orempel/twiboot) flasher for AVR microcontrollers.

## Usage

```bash
# Show bootloader info only (default when no file specified)
twiboot-flasher 0 0x0F

# Basic usage - write firmware to flash
twiboot-flasher 0 0x0F firmware.hex

# Disable verification
twiboot-flasher 0 0x0F firmware.hex -n

# Use different I2C bus
twiboot-flasher 1 0x0F firmware.hex
```

## Command Line Options

- `<BUS>`: I2C bus number (e.g., 0 for /dev/i2c-0) - **Required**
- `<ADDRESS>`: I2C slave address (0x01-0x7F) - **Required**
- `<FILE>`: Firmware file to flash (optional)
- `-n, --no-verify`: Disable verification after write

**Note**: If no file is provided, the tool will show bootloader info and exit.

## File Formats

- **Intel HEX** (`.hex`): Standard Intel HEX format
- **Binary** (`.bin`): Raw binary data
- **Auto-detect**: Automatically detects format based on file extension or content

## Building

```bash
cargo build --release
```

## Requirements

- Linux system with I2C support
- I2C device permissions (usually requires root or i2c group membership)
- Compatible TWI bootloader firmware on target microcontroller

## Example Output

**Info mode (default when no write file specified):**
```
Version: TWIBOOT v3.2
Chip signature: 0x1E 0x93 0x0C
Device: I2C address 0x0F
Flash size: 0x2000 / 8192 bytes (0x40 bytes/page)
```

**Writing firmware:**
```
Version: TWIBOOT v3.2
Chip signature: 0x1E 0x93 0x0C
Device: I2C address 0x0F
Flash size: 0x2000 / 8192 bytes (0x40 bytes/page)
Writing flash from firmware.hex...
Verifying flash...
```

## License

MIT License

