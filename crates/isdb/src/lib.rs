//! ARIBに基づいたMPEG2-TSを読み込むためのクレート。

#![deny(missing_docs)]

pub mod crc32;
pub mod data_module;
pub mod demux;
pub mod desc;
pub mod dsmcc;
pub mod eight;
pub mod filters;
pub mod lang;
pub mod packet;
pub mod pes;
pub mod pid;
pub mod psi;
pub mod table;
pub mod time;
mod utils;

pub use eight::str::{AribStr, AribString};
pub use packet::Packet;
pub use pid::Pid;
