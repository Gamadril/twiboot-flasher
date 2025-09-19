use anyhow::{Result, Context};
use i2cdev::linux::LinuxI2CDevice;
use i2cdev::core::I2CDevice;
use std::time::Duration;
use std::thread;

const WRITE_RETRY_COUNT: usize = 50;
const WRITE_RETRY_DELAY_MS: u64 = 2;

pub struct TwiI2CDevice {
    device: LinuxI2CDevice,
    pub address: u8,
}

impl TwiI2CDevice {
    pub fn new(device_path: &str, address: u8) -> Result<Self> {
        let device = LinuxI2CDevice::new(device_path, address as u16)
            .with_context(|| format!("Failed to open I2C device: {}", device_path))?;

        Ok(TwiI2CDevice { device, address })
    }

    pub fn write_with_retry(&mut self, data: &[u8]) -> Result<()> {
        let mut retries = WRITE_RETRY_COUNT;
        
        loop {
            match self.device.write(data) {
                Ok(_) => return Ok(()),
                Err(e) => {
                    // For I2C, most errors are retryable (slave not acknowledging, etc.)
                    // Only fail immediately for truly fatal errors
                    if retries == 0 {
                        return Err(anyhow::anyhow!("I2C write failed after {} retries: {}", WRITE_RETRY_COUNT, e));
                    }
                }
            }

            retries -= 1;
            thread::sleep(Duration::from_millis(WRITE_RETRY_DELAY_MS));
        }
    }

    pub fn read(&mut self, buffer: &mut [u8]) -> Result<usize> {
        self.device.read(buffer)
            .context("Failed to read from I2C device")?;
        Ok(buffer.len())
    }

    pub fn write_then_read(&mut self, write_data: &[u8], read_buffer: &mut [u8]) -> Result<usize> {
        self.write_with_retry(write_data)?;
        self.read(read_buffer)
    }

    pub fn write_large_data(&mut self, data: &[u8]) -> Result<()> {
        self.write_with_retry(data)
    }
}
