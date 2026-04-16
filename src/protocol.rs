use anyhow::{Context, Result};
use std::thread;
use std::time::Duration;

use crate::i2c::TwiI2CDevice;

// TWI Commands (SLA+R)
const CMD_READ_VERSION: u8 = 0x01;
const CMD_READ_MEMORY: u8 = 0x02;

// TWI Commands (SLA+W)
const CMD_SWITCH_APPLICATION: u8 = 0x01;
const CMD_WRITE_MEMORY: u8 = 0x02;

// Application switch parameters
const BOOTTYPE_BOOTLOADER: u8 = 0x00;
const BOOTTYPE_APPLICATION: u8 = 0x80;

// Memory type parameters
const MEMTYPE_CHIPINFO: u8 = 0x00;
const MEMTYPE_FLASH: u8 = 0x01;

// Block sizes
const READ_BLOCK_SIZE: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddressWidth {
    Bits16,
    Bits32,
}

impl AddressWidth {
    fn bytes(self) -> usize {
        match self {
            AddressWidth::Bits16 => 2,
            AddressWidth::Bits32 => 4,
        }
    }
}

pub struct TwiBootloader {
    i2c: TwiI2CDevice,
    pagesize: u32,
    flashsize: u32,
    address_width: AddressWidth,
}

impl TwiBootloader {
    pub fn new(i2c: TwiI2CDevice) -> Self {
        Self {
            i2c,
            pagesize: 0,
            flashsize: 0,
            address_width: AddressWidth::Bits16,
        }
    }

    pub fn connect(&mut self, wait: bool) -> Result<()> {
        if wait {
            loop {
                match self.try_connect() {
                    Ok(()) => break,
                    Err(e) => {
                        println!(
                            "Connection failed: {}. Retrying in 100ms... (Ctrl+C to cancel)",
                            e
                        );
                        thread::sleep(Duration::from_millis(100));
                    }
                }
            }
        } else {
            self.try_connect()?;
        }
        Ok(())
    }

    fn address_to_bytes(&self, addr: u32) -> Vec<u8> {
        match self.address_width {
            AddressWidth::Bits16 => vec![(addr >> 8) as u8, (addr & 0xFF) as u8],
            AddressWidth::Bits32 => vec![
                ((addr >> 24) & 0xFF) as u8,
                ((addr >> 16) & 0xFF) as u8,
                ((addr >> 8) & 0xFF) as u8,
                (addr & 0xFF) as u8,
            ],
        }
    }

    fn try_connect(&mut self) -> Result<()> {
        // Switch to bootloader mode
        self.switch_application(BOOTTYPE_BOOTLOADER)?;

        // Wait for watchdog and startup time
        thread::sleep(Duration::from_millis(100));

        // Read version
        let version = self.read_version()?;
        println!("Version: {}", version);

        // Set addressing mode from version string
        self.set_address_width_from_version(&version);

        // Read chip info
        let chipinfo = self.read_chipinfo()?;
        self.parse_chipinfo(&chipinfo)?;

        println!("Device: I2C address 0x{:02X}", self.i2c.address);

        Ok(())
    }

    pub fn disconnect(&mut self) -> Result<()> {
        self.switch_application(BOOTTYPE_APPLICATION)
    }

    fn switch_application(&mut self, app_type: u8) -> Result<()> {
        let cmd = [CMD_SWITCH_APPLICATION, app_type];
        self.i2c
            .write_with_retry(&cmd)
            .context("Failed to switch application")?;
        Ok(())
    }

    fn read_version(&mut self) -> Result<String> {
        let cmd = [CMD_READ_VERSION];
        let mut buffer = [0u8; 12];

        self.i2c
            .write_then_read(&cmd, &mut buffer)
            .context("Failed to read version")?;

        // Clear MSB from each byte (as per original code)
        for byte in &mut buffer {
            *byte &= 0x7F;
        }

        // Convert to string - protocol spec says "ASCII, not null terminated"
        // So we read all 16 bytes and trim trailing nulls/spaces
        let version = String::from_utf8_lossy(&buffer)
            .trim_end_matches('\0')
            .trim_end()
            .to_string();

        Ok(version)
    }

    fn read_chipinfo(&mut self) -> Result<[u8; 12]> {
        let mut cmd = Vec::with_capacity(2 + self.address_width.bytes());
        cmd.push(CMD_READ_MEMORY);
        cmd.push(MEMTYPE_CHIPINFO);
        cmd.extend_from_slice(&self.address_to_bytes(0));
        let mut chipinfo = [0u8; 12];

        self.i2c
            .write_then_read(&cmd, &mut chipinfo)
            .context("Failed to read chip info")?;

        Ok(chipinfo)
    }

    fn parse_chipinfo(&mut self, chipinfo: &[u8; 12]) -> Result<()> {
        match self.address_width {
            AddressWidth::Bits16 => {
                // AVR legacy format: 8-byte chipinfo (byte 3 is pagesize)
                self.pagesize = chipinfo[3] as u32;
                self.flashsize = ((chipinfo[4] as u32) << 8) | (chipinfo[5] as u32);
                println!(
                    "Chip signature: 0x{:02X} 0x{:02X} 0x{:02X}",
                    chipinfo[0], chipinfo[1], chipinfo[2]
                );
            }
            AddressWidth::Bits32 => {
                // v4 CH32V format: 4-byte chip ID, 4-byte flash size, 4-byte data
                // In v4, chipinfo[8..10] is a 16-bit pagesize indicator
                self.pagesize = ((chipinfo[8] as u32) << 8) | (chipinfo[9] as u32);
                if self.pagesize == 0 {
                    self.pagesize = 64;
                } // Sanity fallback

                let chip_id = ((chipinfo[0] as u32) << 24)
                    | ((chipinfo[1] as u32) << 16)
                    | ((chipinfo[2] as u32) << 8)
                    | (chipinfo[3] as u32);
                self.flashsize = ((chipinfo[4] as u32) << 24)
                    | ((chipinfo[5] as u32) << 16)
                    | ((chipinfo[6] as u32) << 8)
                    | (chipinfo[7] as u32);

                println!("Chip signature: 0x{:08X}", chip_id);
            }
        }

        println!(
            "Flash size: 0x{:08X} / {} bytes [{}KB] ({} bytes/page)",
            self.flashsize,
            self.flashsize,
            self.flashsize / 1024,
            self.pagesize
        );

        Ok(())
    }

    fn set_address_width_from_version(&mut self, version: &str) {
        // Expect version string like "TWIBOOT v4.0" or "TWIBOOT v3.2"
        if let Some(vpos) = version.find('v') {
            if let Some(ver) = version[vpos + 1..].split_whitespace().next() {
                if let Some(major_str) = ver.split('.').next() {
                    if let Ok(major) = major_str.parse::<u8>() {
                        self.address_width = if major >= 4 {
                            AddressWidth::Bits32
                        } else {
                            AddressWidth::Bits16
                        };
                        return;
                    }
                }
            }
        }

        // Default fallback
        self.address_width = AddressWidth::Bits16;
    }

    pub fn flash_size(&self) -> u32 {
        self.flashsize
    }

    pub fn write_flash(&mut self, data: &[u8]) -> Result<()> {
        let mut pos = 0;

        while pos < data.len() {
            let remaining = data.len() - pos;
            let len = remaining.min(self.pagesize as usize);

            // The bootloader expects exactly one full page in a single I2C transaction
            let addr_bytes = self.address_to_bytes(pos as u32);
            let mut cmd = Vec::with_capacity(2 + addr_bytes.len() + self.pagesize as usize);
            cmd.push(CMD_WRITE_MEMORY);
            cmd.push(MEMTYPE_FLASH);
            cmd.extend_from_slice(&addr_bytes);

            // Add actual data
            cmd.extend_from_slice(&data[pos..pos + len]);

            // Pad with 0xFF to reach exactly pagesize bytes
            let overhead = 2 + addr_bytes.len();
            cmd.resize(overhead + self.pagesize as usize, 0xFF);

            self.i2c
                .write_large_data(&cmd)
                .context("Failed to write flash page")?;

            // Wait for flash programming to complete
            thread::sleep(Duration::from_millis(5));

            pos += len; // Advance by actual data length, not page size
        }

        Ok(())
    }

    pub fn verify_flash(&mut self, expected_data: &[u8]) -> Result<()> {
        // Ensure we're still in bootloader mode before verification
        self.switch_application(BOOTTYPE_BOOTLOADER)?;
        thread::sleep(Duration::from_millis(50));

        let mut pos = 0;

        while pos < expected_data.len() {
            let len = READ_BLOCK_SIZE.min(expected_data.len() - pos);
            let mut buffer = vec![0u8; len];

            let mut cmd = Vec::with_capacity(2 + self.address_width.bytes());
            cmd.push(CMD_READ_MEMORY);
            cmd.push(MEMTYPE_FLASH);
            cmd.extend_from_slice(&self.address_to_bytes(pos as u32));

            // Try to read, if it fails, the device might have switched modes
            match self.i2c.write_then_read(&cmd, &mut buffer) {
                Ok(_) => {}
                Err(_) => {
                    // Device might have switched to application mode, try to switch back
                    self.switch_application(BOOTTYPE_BOOTLOADER)?;
                    thread::sleep(Duration::from_millis(100));
                    self.i2c.write_then_read(&cmd, &mut buffer).context(
                        "Failed to read flash for verification after bootloader re-entry",
                    )?;
                }
            }

            if &buffer[..len] != &expected_data[pos..pos + len] {
                return Err(anyhow::anyhow!(
                    "Verification failed at address 0x{:08X}",
                    pos
                ));
            }

            pos += len;
        }

        Ok(())
    }
}
