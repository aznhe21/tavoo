//! 動画・音声のコーデック。

#![allow(dead_code)]

pub mod audio;
pub mod video;

use std::fmt;

/// 有理数。
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rational<T> {
    /// 分子。
    pub numerator: T,
    /// 分母。
    pub denominator: T,
}

impl<T> Rational<T> {
    /// 分子、分母から有理数を生成する。
    #[inline]
    pub const fn new(numerator: T, denominator: T) -> Rational<T> {
        Rational {
            numerator,
            denominator,
        }
    }

    /// 値を`f32`に変換する。
    #[inline]
    pub fn to_f32(self) -> f32
    where
        T: Into<f32>,
    {
        self.numerator.into() / self.denominator.into()
    }

    /// 値を`f64`に変換する。
    #[inline]
    pub fn to_f64(self) -> f64
    where
        T: Into<f64>,
    {
        self.numerator.into() / self.denominator.into()
    }
}

impl<T: fmt::Display> fmt::Debug for Rational<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}/{}", self.numerator, self.denominator)
    }
}
