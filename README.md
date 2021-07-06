# dfufile

This Rust crate provides tools for processing DFU files as described in the document "Universal Serial Bus Device Class Specification for Device Firmware Upgrade", Revision 1.1 published at <https://usb.org>.

It also supports the extensions added by STMicroelectronics (DfuSe) that are widely used with STM32 microcontrollers, as well as several other products.

## Status

Parsing existing files is fully implemented. Creating new files is not supported yet.

## Binaries

[dfufile-dump](./src/bin/dfufile-dump.rs) is a simple CLI application that dumps the structure of the file given as argument.

## License

Published under the MIT license.

Author: Oliver Rockstedt <info@sourcebox.de>
