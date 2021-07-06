//! Dumps the structure of the file given as argument

use dfufile::DfuFile;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).expect("No file given");
    let mut dfu_file = DfuFile::open(path)?;

    println!("{:#?}", dfu_file);
    println!("Calculated CRC32: {:?}", &mut dfu_file.calc_crc());

    Ok(())
}
