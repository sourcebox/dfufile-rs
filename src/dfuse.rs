//! DfuSe extensions from STMicroelectronics.
//!
//! See document UM0391 Revision 1 for a detailed specification.

use std::io::{Read, Seek};

use crate::Suffix;

use anyhow::{anyhow, Result};

////////////////////////////////////////////////////////////////////////////////

/// Check if the file is a DfuSe file.
pub fn detect(file: &mut std::fs::File) -> Result<bool> {
    file.rewind()?;
    let mut signature = [0; 5];
    file.read_exact(&mut signature)?;

    let suffix = Suffix::from_file(file)?;

    Ok(&signature == b"DfuSe" && suffix.bcdDFU == 0x011A)
}

////////////////////////////////////////////////////////////////////////////////

/// Reference to the file content.
#[derive(Debug)]
pub struct Content {
    /// The prefix header with metadata.
    pub prefix: Prefix,

    /// Vector of contained images.
    pub images: Vec<Image>,
}

impl Content {
    /// Creates a new instance.
    pub fn new(prefix: Prefix, images: Vec<Image>) -> Self {
        Self { prefix, images }
    }

    /// Creates a new instance with data read from file.
    pub fn from_file(file: &mut std::fs::File) -> Result<Self> {
        let file_size = file.seek(std::io::SeekFrom::End(0))?;

        // File must be at least as large as the prefix + standard suffix
        if file_size < (PREFIX_LENGTH + 16) as u64 {
            return Err(anyhow!(Error::InsufficientFileSize));
        }

        let prefix = Prefix::from_file(file)?;
        let mut images = Vec::new();

        let mut file_pos = PREFIX_LENGTH as u64;

        for _ in 0..prefix.bTargets {
            let image = Image::from_file(file, &mut file_pos)?;
            images.push(image);
        }

        let content = Self::new(prefix, images);

        Ok(content)
    }

    /// Find an image with a specific alternate setting.
    pub fn find_image_by_alt(&self, alt_setting: u8) -> Option<&Image> {
        self.images
            .iter()
            .find(|&image| image.target_prefix.bAlternateSetting == alt_setting)
    }

    /// Find an image with a specific name.
    pub fn find_image_by_name<T: AsRef<str>>(&self, name: T) -> Option<&Image> {
        self.images
            .iter()
            .find(|&image| image.target_prefix.szTargetName == name.as_ref())
    }
}

////////////////////////////////////////////////////////////////////////////////

/// Length of the file prefix in bytes.
pub const PREFIX_LENGTH: usize = 11;

/// File prefix, see UM0391 section 2.1.
///
/// The DFU prefix placed as a header is the first part read by the
/// software application, used to retrieve the file context,
/// and enable valid DFU files to be recognized.
#[allow(non_snake_case)]
#[derive(Debug, Clone)]
pub struct Prefix {
    /// File identifier, must contain "DfuSe".
    pub szSignature: String,

    /// Format revision, usually 0x01.
    pub bVersion: u8,

    /// Total file length in bytes (including the suffix).
    pub DFUImageSize: u32,

    /// Number of images stored in the file.
    pub bTargets: u8,
}

impl Default for Prefix {
    /// Creates a new prefix with default values.
    fn default() -> Self {
        Self {
            szSignature: String::from("DfuSe"),
            bVersion: 1,
            DFUImageSize: 0,
            bTargets: 0,
        }
    }
}

impl Prefix {
    /// Creates a new prefix.
    pub fn new(signature: String, version: u8, image_size: u32, num_targets: u8) -> Self {
        Self {
            szSignature: signature,
            bVersion: version,
            DFUImageSize: image_size,
            bTargets: num_targets,
        }
    }

    /// Creates a new prefix from a buffer of u8 values.
    pub fn from_bytes(buffer: &[u8; PREFIX_LENGTH]) -> Self {
        Self::new(
            String::from_utf8_lossy(&buffer[0..5]).to_string(),
            u8::from_le(buffer[5]),
            u32::from_le_bytes([buffer[6], buffer[7], buffer[8], buffer[9]]),
            u8::from_le(buffer[10]),
        )
    }

    /// Creates a new prefix from reading a file.
    pub fn from_file(file: &mut std::fs::File) -> Result<Self> {
        file.rewind()?;
        let mut buffer = [0; PREFIX_LENGTH];
        file.read_exact(&mut buffer)?;

        let data = Self::from_bytes(&buffer);

        if &data.szSignature != "DfuSe" {
            return Err(anyhow!(Error::InvalidPrefixSignature));
        }

        Ok(data)
    }
}

////////////////////////////////////////////////////////////////////////////////

/// An image, see UM0391 section 2.3.1.
///
/// The DFU Image contains the effective data of the firmware,
/// starting by a Target prefix record followed by a number of Image elements
#[derive(Debug, Clone)]
pub struct Image {
    /// Target prefix record containing metadata.
    pub target_prefix: TargetPrefix,

    /// Vector of image elements containing the data.
    pub image_elements: Vec<ImageElement>,
}

impl Default for Image {
    /// Creates a new image with default values.
    fn default() -> Self {
        Self {
            target_prefix: TargetPrefix::default(),
            image_elements: Vec::new(),
        }
    }
}

impl Image {
    /// Creates a new image.
    pub fn new(target_prefix: TargetPrefix, image_elements: Vec<ImageElement>) -> Self {
        Self {
            target_prefix,
            image_elements,
        }
    }

    /// Creates a new image by reading a file.
    ///
    /// The `file_pos` argument must be set to the postion inside the file as
    /// offset from the start and is updated according to the number of bytes read.
    pub fn from_file(file: &mut std::fs::File, file_pos: &mut u64) -> Result<Self> {
        let target_prefix = TargetPrefix::from_file(file, file_pos)?;
        let mut image_elements = Vec::new();

        for _ in 0..target_prefix.dwNbElements {
            let image_element = ImageElement::from_file(file, file_pos)?;
            image_elements.push(image_element);
        }

        let image = Image::new(target_prefix, image_elements);

        Ok(image)
    }
}

////////////////////////////////////////////////////////////////////////////////

/// Length of the target prefix in bytes.
pub const TARGET_PREFIX_LENGTH: usize = 274;

/// Target prefix of an image, see UM0391 section 2.3.2.
///
/// The target prefix record is used to describe the associated image
#[allow(non_snake_case)]
#[derive(Debug, Clone)]
pub struct TargetPrefix {
    /// Target identifier, must contain "Target".
    pub szSignature: String,

    /// The device's alternate setting for which this image is intended.
    pub bAlternateSetting: u8,

    /// Boolean value (0 or 1) which indicates if the target is named or not.
    pub bTargetNamed: u8,

    /// Target name.
    pub szTargetName: String,

    /// Whole length of the associated image excluding this target prefix.
    pub dwTargetSize: u32,

    /// Number of elements in the associated image.
    pub dwNbElements: u32,
}

impl Default for TargetPrefix {
    /// Creates a new target prefix with default values.
    fn default() -> Self {
        Self {
            szSignature: String::from("Target"),
            bAlternateSetting: 0,
            bTargetNamed: 0,
            szTargetName: String::new(),
            dwTargetSize: 0,
            dwNbElements: 0,
        }
    }
}

impl TargetPrefix {
    /// Creates a new target prefix.
    pub fn new(
        signature: String,
        alt_setting: u8,
        named: u8,
        target_name: String,
        target_size: u32,
        num_elements: u32,
    ) -> Self {
        Self {
            szSignature: signature,
            bAlternateSetting: alt_setting,
            bTargetNamed: named,
            szTargetName: target_name,
            dwTargetSize: target_size,
            dwNbElements: num_elements,
        }
    }

    /// Creates a new target prefix from a buffer of u8 values.
    pub fn from_bytes(buffer: &[u8; TARGET_PREFIX_LENGTH]) -> Self {
        // The target name in the buffer is a null-terminated C string
        // but often the rest of the buffer contains garbage.
        // So we do some extra work here to detect the real length used.
        let target_name_full = String::from_utf8_lossy(&buffer[11..266]).to_string();
        let target_name_len = target_name_full.find('\x00');

        // If no null character is found, length is set to maximum of 255.
        let target_name_len = target_name_len.unwrap_or(255);

        Self::new(
            String::from_utf8_lossy(&buffer[0..6]).to_string(),
            u8::from_le(buffer[6]),
            u8::from_le(buffer[7]),
            String::from_utf8_lossy(&buffer[11..266])[0..target_name_len].to_string(),
            u32::from_le_bytes([buffer[266], buffer[267], buffer[268], buffer[269]]),
            u32::from_le_bytes([buffer[270], buffer[271], buffer[272], buffer[273]]),
        )
    }

    /// Creates a new target prefix by reading a file.
    ///
    /// The `file_pos` argument must be set to the postion inside the file as
    /// offset from the start and is updated according to the number of bytes read.
    pub fn from_file(file: &mut std::fs::File, file_pos: &mut u64) -> Result<Self> {
        file.seek(std::io::SeekFrom::Start(*file_pos))?;
        let mut buffer = [0; TARGET_PREFIX_LENGTH];
        file.read_exact(&mut buffer)?;

        *file_pos += TARGET_PREFIX_LENGTH as u64;

        let data = Self::from_bytes(&buffer);

        if &data.szSignature != "Target" {
            return Err(anyhow!(Error::InvalidTargetPrefixSignature));
        }

        Ok(data)
    }
}

////////////////////////////////////////////////////////////////////////////////

/// Length of the image element without data in bytes.
pub const IMAGE_ELEMENT_LENGTH: usize = 8;

/// An image element, see UM0391 section 2.3.3.
///
/// The image element provides a data record containing the effective
/// firmware data preceded by the data address and data size.
#[allow(non_snake_case)]
#[derive(Debug, Clone)]
pub struct ImageElement {
    /// Starting address of the data.
    pub dwElementAddress: u32,

    /// Size of the contained data.
    pub dwElementSize: u32,

    /// File position of data as offset from the start.
    pub data_position: u64,
}

impl Default for ImageElement {
    /// Creates a new image element with default values.
    fn default() -> Self {
        Self {
            dwElementAddress: 0,
            dwElementSize: 0,
            data_position: 0,
        }
    }
}

impl ImageElement {
    /// Creates a new image element.
    pub fn new(element_address: u32, element_size: u32, data_position: u64) -> Self {
        Self {
            dwElementAddress: element_address,
            dwElementSize: element_size,
            data_position,
        }
    }

    /// Creates a new image element from a buffer of u8 values and data position.
    pub fn from_bytes(buffer: &[u8; IMAGE_ELEMENT_LENGTH], data_position: u64) -> Self {
        Self::new(
            u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]),
            u32::from_le_bytes([buffer[4], buffer[5], buffer[6], buffer[7]]),
            data_position,
        )
    }

    /// Creates a new image element by reading a file.
    ///
    /// The `file_pos` argument must be set to the postion inside the file as
    /// offset from the start and is updated according to the number of bytes read.
    pub fn from_file(file: &mut std::fs::File, file_pos: &mut u64) -> Result<Self> {
        file.seek(std::io::SeekFrom::Start(*file_pos))?;
        let mut buffer = [0; IMAGE_ELEMENT_LENGTH];
        file.read_exact(&mut buffer)?;

        *file_pos += IMAGE_ELEMENT_LENGTH as u64;

        let data = Self::from_bytes(&buffer, *file_pos);

        *file_pos += data.dwElementSize as u64;

        Ok(data)
    }

    /// Read data from file into a buffer.
    ///
    /// The `position` argument is relative to the start of the element
    /// The function tries to fill the buffer completely and returns the
    /// number of valid bytes in the buffer. This may be less than the buffer
    /// size in case of EOF or reaching the element borders.
    pub fn read_at(
        &self,
        file: &mut std::fs::File,
        position: u32,
        buffer: &mut [u8],
    ) -> Result<usize> {
        let file_pos = self.data_position + (position as u64);
        file.seek(std::io::SeekFrom::Start(file_pos))?;
        let read_size = file.read(buffer)?;

        let read_size = std::cmp::min(read_size, (self.dwElementSize - position) as usize);

        Ok(read_size)
    }
}

////////////////////////////////////////////////////////////////////////////////

/// Parsing errors.
#[derive(Debug)]
pub enum Error {
    /// File prefix signature is not "DfuSe".
    InvalidPrefixSignature,

    /// Target prefix signature is not "Target".
    InvalidTargetPrefixSignature,

    /// File is too small (smaller than prefix + suffix size).
    InsufficientFileSize,
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::InvalidPrefixSignature => "Invalid file prefix signature",
                Self::InvalidTargetPrefixSignature => "Invalid target prefix signature",
                Self::InsufficientFileSize => "File size is to small to contain prefix and suffix",
            }
        )
    }
}
