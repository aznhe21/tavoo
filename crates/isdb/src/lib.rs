//! ARIBに基づいたMPEG2-TSを読み込むためのクレート。

#![deny(missing_docs)]

pub mod crc32;
pub mod demux;
pub mod desc;
pub mod packet;
pub mod pid;
pub mod psi;
pub mod table;
pub mod time;
mod utils;

pub use packet::Packet;
pub use pid::Pid;
