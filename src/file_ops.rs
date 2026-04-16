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
    flash_limit: u32,
) -> Result<Vec<u8>> {
    let data =
        fs::read(path).with_context(|| format!("Failed to read file: {}", path.display()))?;

    match format {
        FileFormat::Binary => Ok(data),
        FileFormat::Hex => parse_hex_file(&data, Some(flash_limit)),
        FileFormat::Auto => {
            // Try to detect format
            if data.starts_with(b":") {
                parse_hex_file(&data, Some(flash_limit))
            } else {
                Ok(data)
            }
        }
    }
}

fn parse_hex_file(data: &[u8], flash_limit: Option<u32>) -> Result<Vec<u8>> {
    let content = String::from_utf8(data.to_vec()).context("Invalid UTF-8 in hex file")?;

    // Use provided flash limit or default to ATtiny84 layout for backward compatibility
    let flash_limit = flash_limit.unwrap_or(0x1C00);
    let max_app_size = flash_limit as usize;

    let mut result = vec![0xFF; max_app_size]; // Initialize with 0xFF (erased flash)
    let mut max_address = 0u32;

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
            u16::from_str_radix(&hex_data[2..6], 16).context("Invalid address in hex file")? as u32;

        let record_type =
            u8::from_str_radix(&hex_data[6..8], 16).context("Invalid record type in hex file")?;

        match record_type {
            0x00 => {
                // Data record
                if hex_data.len() < (8 + byte_count as usize * 2) {
                    continue; // Skip invalid data records
                }

                // Check if address conflicts with bootloader space
                if address >= flash_limit {
                    return Err(anyhow::anyhow!(
                        "HEX file contains data at address 0x{:04X} which exceeds available flash space (limit: 0x{:04X}).",
                        address, flash_limit
                    ));
                }

                let data_start = 8;
                let data_end = data_start + (byte_count as usize * 2);

                for (i, byte_offset) in (data_start..data_end).step_by(2).enumerate() {
                    let byte_str = &hex_data[byte_offset..byte_offset + 2];
                    let byte = u8::from_str_radix(byte_str, 16)
                        .context("Invalid data byte in hex file")?;

                    let target_addr = address + i as u32;
                    if target_addr < flash_limit {
                        result[target_addr as usize] = byte;
                    }
                }

                max_address = max_address.max(address + byte_count as u32);
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
