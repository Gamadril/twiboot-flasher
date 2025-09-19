use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub enum FileFormat {
    Binary,
    Hex,
    Auto,
}

impl FileFormat {
    pub fn from_extension(path: &Path) -> Self {
        match path.extension().and_then(|s| s.to_str()) {
            Some("hex") => FileFormat::Hex,
            Some("bin") => FileFormat::Binary,
            _ => FileFormat::Auto,
        }
    }
}

pub fn read_file_with_bootloader_info(
    path: &Path,
    format: FileFormat,
    bootloader_start: u16,
) -> Result<Vec<u8>> {
    let data =
        fs::read(path).with_context(|| format!("Failed to read file: {}", path.display()))?;

    match format {
        FileFormat::Binary => Ok(data),
        FileFormat::Hex => parse_hex_file(&data, Some(bootloader_start)),
        FileFormat::Auto => {
            // Try to detect format
            if data.starts_with(b":") {
                parse_hex_file(&data, Some(bootloader_start))
            } else {
                Ok(data)
            }
        }
    }
}

fn parse_hex_file(data: &[u8], bootloader_start: Option<u16>) -> Result<Vec<u8>> {
    let content = String::from_utf8(data.to_vec()).context("Invalid UTF-8 in hex file")?;

    // Use provided bootloader start or default to ATtiny84 layout for backward compatibility
    let bootloader_start = bootloader_start.unwrap_or(0x1C00);
    let max_app_size = bootloader_start as usize;

    let mut result = vec![0xFF; max_app_size]; // Initialize with 0xFF (erased flash)
    let mut max_address = 0u16;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with(':') {
            continue;
        }

        let hex_data = &line[1..]; // Remove ':'
        if hex_data.len() < 8 {
            continue; // Skip invalid lines
        }

        let byte_count =
            u8::from_str_radix(&hex_data[0..2], 16).context("Invalid byte count in hex file")?;

        let address =
            u16::from_str_radix(&hex_data[2..6], 16).context("Invalid address in hex file")?;

        let record_type =
            u8::from_str_radix(&hex_data[6..8], 16).context("Invalid record type in hex file")?;

        match record_type {
            0x00 => {
                // Data record
                if hex_data.len() < (8 + byte_count as usize * 2) {
                    continue; // Skip invalid data records
                }

                // Check if address conflicts with bootloader space
                if address >= bootloader_start {
                    return Err(anyhow::anyhow!(
                        "HEX file contains data at address 0x{:04X} which conflicts with bootloader space (0x{:04X}-0xFFFF). \
                        Application firmware should only use addresses 0x0000-0x{:04X}",
                        address, bootloader_start, bootloader_start - 1
                    ));
                }

                let data_start = 8;
                let data_end = data_start + (byte_count as usize * 2);

                for (i, byte_offset) in (data_start..data_end).step_by(2).enumerate() {
                    let byte_str = &hex_data[byte_offset..byte_offset + 2];
                    let byte = u8::from_str_radix(byte_str, 16)
                        .context("Invalid data byte in hex file")?;

                    let target_addr = address + i as u16;
                    if target_addr < bootloader_start {
                        result[target_addr as usize] = byte;
                    }
                }

                max_address = max_address.max(address + byte_count as u16);
            }
            0x01 => {
                // End of file record
                break;
            }
            _ => {
                // Skip other record types
                continue;
            }
        }
    }

    // Trim result to actual data size (remove trailing 0xFF)
    let actual_size = (max_address as usize).min(max_app_size);
    result.truncate(actual_size);

    Ok(result)
}
