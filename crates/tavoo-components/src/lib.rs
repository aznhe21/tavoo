//! TaVooの構成要素を分離しておくためのクレート。

#![deny(missing_docs, unsafe_op_in_unsafe_fn)]

#[macro_use]
mod macros;

pub mod extract;
pub mod player;
mod sys;
pub mod webview;
