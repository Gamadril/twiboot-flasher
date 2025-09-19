use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

mod i2c;
mod protocol;
mod file_ops;

use i2c::TwiI2CDevice;
use protocol::TwiBootloader;
use file_ops::{FileFormat, read_file_with_bootloader_info};

#[derive(Parser)]
#[command(name = "twiboot-flasher")]
#[command(about = "TWI/I2C bootloader flasher for AVR microcontrollers")]
#[command(version)]
struct Cli {
    /// I2C bus number (e.g., 0 for /dev/i2c-0)
    bus: u8,

    /// I2C slave address (0x01-0x7F)
    #[arg(value_parser = parse_address)]
    address: u8,

    /// Firmware file to flash (optional - if not provided, shows bootloader info)
    #[arg(value_name = "FILE")]
    file: Option<String>,

    /// Disable verify after write
    #[arg(short = 'n', long = "no-verify")]
    no_verify: bool,

}

fn parse_address(s: &str) -> Result<u8, String> {
    if let Some(hex_str) = s.strip_prefix("0x") {
        u8::from_str_radix(hex_str, 16)
            .map_err(|_| format!("Invalid hex address: {}", s))
    } else {
        s.parse::<u8>()
            .map_err(|_| format!("Invalid address: {}", s))
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.address < 0x01 || cli.address > 0x7F {
        eprintln!("Error: I2C address must be between 0x01 and 0x7F");
        std::process::exit(1);
    }

    // Create device path from bus number
    let device_path = format!("/dev/i2c-{}", cli.bus);

    // Create I2C device
    let i2c = TwiI2CDevice::new(&device_path, cli.address)?;
    
    // Create bootloader instance
    let mut bootloader = TwiBootloader::new(i2c);

    // Connect to bootloader
    bootloader.connect()?;


    // If no file specified, just show info and exit
    if cli.file.is_none() {
        // Info is already displayed in connect(), just exit
        return Ok(());
    }

    // Process write operation
    if let Some(filename) = &cli.file {
        let filepath = PathBuf::from(filename);
        
        if !filepath.exists() {
            eprintln!("Error: File not found: {}", filepath.display());
            std::process::exit(1);
        }

        println!("Writing flash from {}", filepath.display());
        let bootloader_start = bootloader.get_bootloader_start();
        let data = read_file_with_bootloader_info(&filepath, FileFormat::from_extension(&filepath), bootloader_start)?;
        bootloader.write_flash(&data)?;

        if !cli.no_verify {
            println!("Verifying flash...");
            bootloader.verify_flash(&data)?;
        }
    }

    // Disconnect (switch to application)
    bootloader.disconnect()?;

    Ok(())
}
