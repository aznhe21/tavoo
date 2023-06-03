//! TaVooの構成要素を分離しておくためのクレート。

#![deny(missing_docs, unsafe_op_in_unsafe_fn)]

#[macro_use]
mod macros;

pub mod bit;
pub mod codec;
pub mod extract;
pub mod player;
pub mod ring_buf;
mod sys;
pub mod webview;
