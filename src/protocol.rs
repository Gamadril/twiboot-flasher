use anyhow::{Result, Context};
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

pub struct TwiBootloader {
    i2c: TwiI2CDevice,
    pagesize: u8,
    flashsize: u16,
}

impl TwiBootloader {
    pub fn new(i2c: TwiI2CDevice) -> Self {
        Self {
            i2c,
            pagesize: 0,
            flashsize: 0,
        }
    }

    pub fn connect(&mut self) -> Result<()> {
        // Switch to bootloader mode
        self.switch_application(BOOTTYPE_BOOTLOADER)?;
        
        // Wait for watchdog and startup time
        thread::sleep(Duration::from_millis(100));

        // Read version
        let version = self.read_version()?;
        println!("Version: {}", version);

        // Read chip info
        let chipinfo = self.read_chipinfo()?;
        self.parse_chipinfo(&chipinfo)?;

        println!("Device: I2C address 0x{:02X}", self.i2c.address);
        println!("Flash size: 0x{:04X} / {} bytes (0x{:02X} bytes/page)",
                 self.flashsize, self.flashsize, self.pagesize);
        println!("Bootloader start: 0x{:04X} (as provided by the device)", self.get_bootloader_start());

        Ok(())
    }

    pub fn disconnect(&mut self) -> Result<()> {
        self.switch_application(BOOTTYPE_APPLICATION)
    }

    fn switch_application(&mut self, app_type: u8) -> Result<()> {
        let cmd = [CMD_SWITCH_APPLICATION, app_type];
        self.i2c.write_with_retry(&cmd)
            .context("Failed to switch application")?;
        Ok(())
    }

    fn read_version(&mut self) -> Result<String> {
        let cmd = [CMD_READ_VERSION];
        let mut buffer = [0u8; 16];
        
        self.i2c.write_then_read(&cmd, &mut buffer)
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

    fn read_chipinfo(&mut self) -> Result<[u8; 8]> {
        let cmd = [CMD_READ_MEMORY, MEMTYPE_CHIPINFO, 0x00, 0x00];
        let mut chipinfo = [0u8; 8];
        
        self.i2c.write_then_read(&cmd, &mut chipinfo)
            .context("Failed to read chip info")?;

        Ok(chipinfo)
    }

    fn parse_chipinfo(&mut self, chipinfo: &[u8; 8]) -> Result<()> {
        // chipinfo format: [sig0, sig1, sig2, pagesize, flash_hi, flash_lo, eeprom_hi, eeprom_lo]
        self.pagesize = chipinfo[3];
        self.flashsize = ((chipinfo[4] as u16) << 8) | (chipinfo[5] as u16);
        
        println!("Chip signature: 0x{:02X} 0x{:02X} 0x{:02X}", 
                 chipinfo[0], chipinfo[1], chipinfo[2]);
        
        Ok(())
    }

    pub fn get_bootloader_start(&self) -> u16 {
        // The flashsize field actually contains the bootloader start address
        // (this is a bit confusing naming, but that's how the original protocol works)
        self.flashsize
    }

    pub fn write_flash(&mut self, data: &[u8]) -> Result<()> {
        let mut pos = 0;

        while pos < data.len() {
            let remaining = data.len() - pos;
            let len = remaining.min(self.pagesize as usize);
            
            // The bootloader expects exactly one full page in a single I2C transaction
            let mut cmd = Vec::with_capacity(4 + self.pagesize as usize);
            cmd.extend_from_slice(&[
                CMD_WRITE_MEMORY,
                MEMTYPE_FLASH,
                (pos >> 8) as u8,
                (pos & 0xFF) as u8,
            ]);

            // Add actual data
            cmd.extend_from_slice(&data[pos..pos + len]);
            
            // Pad with 0xFF to reach exactly pagesize bytes
            cmd.resize(4 + self.pagesize as usize, 0xFF);
            
            self.i2c.write_large_data(&cmd)
                .context("Failed to write flash page")?;

            // Wait for flash programming to complete (C version has no delay)
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

            let cmd = [
                CMD_READ_MEMORY,
                MEMTYPE_FLASH,
                (pos >> 8) as u8,
                (pos & 0xFF) as u8,
            ];

            // Try to read, if it fails, the device might have switched modes
            match self.i2c.write_then_read(&cmd, &mut buffer) {
                Ok(_) => {},
                Err(_) => {
                    // Device might have switched to application mode, try to switch back
                    self.switch_application(BOOTTYPE_BOOTLOADER)?;
                    thread::sleep(Duration::from_millis(100));
                    self.i2c.write_then_read(&cmd, &mut buffer)
                        .context("Failed to read flash for verification after bootloader re-entry")?;
                }
            }

            if &buffer[..len] != &expected_data[pos..pos + len] {
                return Err(anyhow::anyhow!("Verification failed at address 0x{:04X}", pos));
            }

            pos += len;
        }

        Ok(())
    }
}
