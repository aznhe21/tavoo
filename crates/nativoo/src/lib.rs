//! TaVooの構成要素を分離しておくためのクレート。

#![deny(missing_docs, unsafe_op_in_unsafe_fn)]

#[macro_use]
mod macros;

mod bit;
mod codec;
mod extract;
mod ring_buf;
mod sys;

pub mod player;
pub mod webview;
