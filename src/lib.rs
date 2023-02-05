//! This crate offers tools for processing DFU files as described
//! in the document "Universal Serial Bus Device Class Specification for
//! Device Firmware Upgrade", Revision 1.1 published at <https://usb.org>
//!
//! It also supports the extensions added by STMicroelectronics (DfuSe)
//! that are widely used with STM32 microcontrollers, as well as
//! several other products.

pub mod crc32;
pub mod dfuse;

use std::io::{Read, Seek};

use anyhow::{anyhow, Result};

////////////////////////////////////////////////////////////////////////////////

/// File handle
#[derive(Debug)]
pub struct DfuFile {
    /// Reference to the file on the filesystem.
    pub file: std::fs::File,

    /// Path to the file.
    pub path: std::path::PathBuf,

    /// The content representation.
    pub content: Content,

    /// The file suffix with meta information.
    pub suffix: Suffix,
}

impl DfuFile {
    /// Creates a new instance.
    pub fn new(
        file: std::fs::File,
        path: std::path::PathBuf,
        content: Content,
        suffix: Suffix,
    ) -> Self {
        Self {
            file,
            path,
            content,
            suffix,
        }
    }

    /// Open existing file.
    pub fn open<P: AsRef<std::path::Path> + Clone>(path: P) -> Result<Self> {
        let mut file = std::fs::File::open(path.clone())?;

        let file_size = file.seek(std::io::SeekFrom::End(0))?;

        // File must be at least as large as the suffix
        if file_size < SUFFIX_LENGTH as u64 {
            return Err(anyhow!(Error::InsufficientFileSize));
        }

        let content = if dfuse::detect(&mut file)? {
            Content::DfuSe(dfuse::Content::from_file(&mut file)?)
        } else {
            Content::Plain
        };

        let suffix = Suffix::from_file(&mut file)?;

        Ok(Self::new(
            file,
            std::path::PathBuf::from(path.as_ref()),
            content,
            suffix,
        ))
    }

    /// Calculate the CRC32 checksum of whole file excluding the last 4 bytes,
    /// which contain the checksum itself.
    pub fn calc_crc(&mut self) -> Result<u32> {
        let file_size = self.file.seek(std::io::SeekFrom::End(0))?;
        self.file.rewind()?;

        const CHUNK_SIZE: u64 = 1024;
        let mut file_pos = 0;
        let mut crc = 0;

        loop {
            let read_size = std::cmp::min(CHUNK_SIZE, file_size - 4 - file_pos);

            if read_size == 0 {
                break;
            }

            let mut buffer = vec![0; read_size as usize];
            self.file.read_exact(&mut buffer)?;

            crc = crc32::crc32(&buffer, crc);

            file_pos += read_size;
        }

        Ok(crc ^ 0xFFFFFFFF_u32)
    }
}

////////////////////////////////////////////////////////////////////////////////

/// File content variants.
#[derive(Debug)]
pub enum Content {
    /// Standard file with raw content.
    Plain,

    /// DfuSe file with extensions from STMicroelectronics.
    DfuSe(dfuse::Content),
}

impl std::fmt::Display for Content {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Plain => "Plain".to_string(),
                Self::DfuSe(content) => format!("DfuSe v{}", content.prefix.bVersion),
            }
        )
    }
}

////////////////////////////////////////////////////////////////////////////////

/// Length of the file suffix in bytes.
pub const SUFFIX_LENGTH: usize = 16;

/// File suffix containing the metadata.
#[allow(non_snake_case)]
#[derive(Debug, Clone)]
pub struct Suffix {
    /// Firmware version contained in the file, or 0xFFFF if ignored.
    pub bcdDevice: u16,

    /// Intended product id of the device or 0xFFFF if the field is ignored.
    pub idProduct: u16,

    /// Intended vendor id of the device or 0xFFFF if the field is ignored.
    pub idVendor: u16,

    /// DFU specification number.
    /// - 0x0100 for standard files.
    /// - 0x011A for DfuSe files.
    pub bcdDFU: u16,

    /// File identifier, must contain "DFU" in reversed order.
    pub ucDFUSignature: String,

    /// Length of the suffix itself, fixed to 16.
    pub bLength: u8,

    /// Calculated CRC32 over the whole file except for the dwCRC data itself.
    pub dwCRC: u32,
}

impl Default for Suffix {
    /// Creates a new suffix with default values.
    fn default() -> Self {
        Self {
            bcdDevice: 0xFFFF,
            idProduct: 0xFFFF,
            idVendor: 0xFFFF,
            bcdDFU: 0x0100,
            ucDFUSignature: String::from("UFD"),
            bLength: SUFFIX_LENGTH as u8,
            dwCRC: 0,
        }
    }
}

impl Suffix {
    /// Creates a new suffix.
    pub fn new(
        device_version: u16,
        product_id: u16,
        vendor_id: u16,
        dfu_spec_no: u16,
        signature: String,
        length: u8,
        crc: u32,
    ) -> Self {
        Self {
            bcdDevice: device_version,
            idProduct: product_id,
            idVendor: vendor_id,
            bcdDFU: dfu_spec_no,
            ucDFUSignature: signature,
            bLength: length,
            dwCRC: crc,
        }
    }

    /// Creates a new suffix from a buffer of u8 values.
    pub fn from_bytes(buffer: &[u8; SUFFIX_LENGTH]) -> Self {
        Self::new(
            u16::from_le_bytes([buffer[0], buffer[1]]),
            u16::from_le_bytes([buffer[2], buffer[3]]),
            u16::from_le_bytes([buffer[4], buffer[5]]),
            u16::from_le_bytes([buffer[6], buffer[7]]),
            String::from_utf8_lossy(&buffer[8..11]).to_string(),
            u8::from_le(buffer[11]),
            u32::from_le_bytes([buffer[12], buffer[13], buffer[14], buffer[15]]),
        )
    }

    /// Creates a new suffix from reading a file.
    pub fn from_file(file: &mut std::fs::File) -> Result<Self> {
        file.seek(std::io::SeekFrom::End(-(SUFFIX_LENGTH as i64)))?;
        let mut buffer = [0; SUFFIX_LENGTH];
        file.read_exact(&mut buffer)?;

        let data = Self::from_bytes(&buffer);

        if &data.ucDFUSignature != "UFD" {
            return Err(anyhow!(Error::InvalidSuffixSignature));
        }

        Ok(data)
    }
}

////////////////////////////////////////////////////////////////////////////////

/// Parsing errors.
#[derive(Debug)]
pub enum Error {
    /// File suffix signature is not "UFD" (DFU reversed).
    InvalidSuffixSignature,

    /// File is too small (smaller than suffix size).
    InsufficientFileSize,
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::InvalidSuffixSignature => "Invalid file suffix signature",
                Self::InsufficientFileSize => "File size is to small to contain suffix",
            }
        )
    }
}
