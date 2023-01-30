//! ARIBに基づいたMPEG2-TSを読み込むためのクレート。

#![deny(missing_docs)]

pub mod packet;
pub mod pid;
mod utils;

pub use packet::Packet;
pub use pid::Pid;
