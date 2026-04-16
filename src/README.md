# TWI Bootloader Flasher

Rust implementation of the TWI/I2C bootloader flasher supporting both AVR microcontrollers (16-bit addressing) and WCH CH32V microcontrollers (32-bit addressing).

## Usage

```bash
# Show bootloader info only (default when no file specified)
twiboot-flasher 0 0x0F

# Basic usage - write firmware to flash
twiboot-flasher 0 0x0F firmware.hex

# Wait mode - retry connection every 100ms (useful for devices that need time to boot)
twiboot-flasher 0 0x0F firmware.hex --wait

# Disable verification
twiboot-flasher 0 0x0F firmware.hex -n

# Use different I2C bus
twiboot-flasher 1 0x0F firmware.hex
```

## Command Line Options

- `<BUS>`: I2C bus number (e.g., 0 for /dev/i2c-0) - **Required**
- `<ADDRESS>`: I2C slave address (0x01-0x7F) - **Required**
- `<FILE>`: Firmware file to flash (optional)
- `-w, --wait`: Retry connection every 100ms until device responds
- `-n, --no-verify`: Disable verification after write

**Note**: If no file is provided, the tool will show bootloader info and exit. Flash/chipinfo **address width** (16 vs 32 bit on the bus) is chosen automatically from the reported TWIBOOT version, not from a flag (see **Address width**).

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

## Address width (16-bit vs 32-bit)

The flasher does **not** take a CLI flag for this. After reading the version string from the device (e.g. `TWIBOOT v3.2` or `TWIBOOT v4.0`), it picks the on-wire address format for read/write/chipinfo:

| Bootloader major version | Address bytes per flash/chipinfo access |
|--------------------------|----------------------------------------|
| **v4 and newer**         | **4** (32-bit, big-endian on the bus) |
| **v3.x and older**       | **2** (16-bit, high byte first)       |

If the version string cannot be parsed (no `v<major>.<minor>` pattern), the tool **falls back to 16-bit** addressing.

That lines up with typical targets: **AVR** TWIBOOT builds are usually v3.x (16-bit); **CH32V** TWIBOOT v4.x uses the 32-bit layout and chipinfo parsing in the same connect path.

### AVR (16-bit protocol path)

- Chipinfo and flash addresses use **2** payload bytes per transaction.
- Intel HEX loading still clips to the reported flash size (bootloader region at the end of flash stays out of the image the same way as before).

### CH32V / v4 (32-bit protocol path)

- Chipinfo and flash addresses use **4** payload bytes per transaction.
- Chipinfo layout differs from the legacy AVR 8-byte form (see code / device docs); flash capacity comes from the v4 chipinfo block.

## Example Output

**Info mode (AVR):**
```
Version: TWIBOOT v3.2
Chip signature: 0x1E 0x93 0x0C
Device: I2C address 0x0F
Flash size: 0x00002000 / 8192 bytes [8KB] (64 bytes/page)
```

**Info mode (CH32V):**
```
Version: TWIBOOT v4.0
Chip signature: 0xB5252020
Device: I2C address 0x29
Flash size: 0x00004000 / 16384 bytes [16KB] (64 bytes/page)
```

**Write mode:**
```
Writing flash from firmware.hex
Verifying flash...
```

**Writing firmware:**
```
Version: TWIBOOT v3.2
Chip signature: 0x1E 0x93 0x0C
Device: I2C address 0x0F
Flash size: 0x00002000 / 8192 bytes [8KB] (64 bytes/page)
Writing flash from firmware.hex...
Verifying flash...
```

## License

MIT License

