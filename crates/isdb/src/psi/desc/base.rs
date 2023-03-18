//! 記述子に関する基礎の型。

use std::fmt;

use crate::utils::{BytesExt, SliceExt};

/// 記述子を表すトレイト。
pub trait Descriptor<'a>: Sized {
    /// この記述子のタグ。
    const TAG: u8;

    /// `data`から記述子を読み取る。
    ///
    /// `data`には`descriptor_tag`と`descriptor_length`は含まない。
    fn read(data: &'a [u8]) -> Option<Self>;
}

/// パース前の記述子。
pub struct RawDescriptor<'a> {
    /// 記述子のタグ。
    pub tag: u8,

    /// 記述子の内容。
    pub data: &'a [u8],
}

impl<'a> fmt::Debug for RawDescriptor<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        struct PrintBytes<'a>(&'a [u8]);
        impl<'a> fmt::Debug for PrintBytes<'a> {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "{} bytes", self.0.len())
            }
        }

        f.debug_struct("RawDescriptor")
            .field("tag", &crate::utils::UpperHex(self.tag))
            .field("data", &PrintBytes(self.data))
            .finish()
    }
}

/// 複数の記述子からなる記述子群。
#[derive(Clone, PartialEq, Eq)]
pub struct DescriptorBlock<'a>(&'a [u8]);

impl<'a> DescriptorBlock<'a> {
    /// `data`から`length`バイト分の記述子群を読み取り後続データと共に返す。
    ///
    /// 記述子の内容はパースせず、`get`メソッドで初めてパースする。
    ///
    /// データ長が不足している場合は`None`を返す。
    // `length`が`u16`なのは規格上`u16`以上の長さになることがなく、
    // 呼び出し側でのキャストが無意味であるため。
    pub fn read_with_len(data: &'a [u8], length: u16) -> Option<(DescriptorBlock<'a>, &'a [u8])> {
        let (block, rem) = data.split_at_checked(length as usize)?;
        Some((DescriptorBlock(block), rem))
    }

    /// `data`から記述子群を読み取り後続データと共に返す。
    ///
    /// 記述子の内容はパースせず、`get`メソッドで初めてパースする。
    ///
    /// データ長が不足している場合は`None`を返す。
    #[inline]
    pub fn read(data: &'a [u8]) -> Option<(DescriptorBlock<'a>, &'a [u8])> {
        if data.len() < 2 {
            return None;
        }

        let length = data[0..=1].read_be_16() & 0b0000_1111_1111_1111;
        DescriptorBlock::read_with_len(&data[2..], length)
    }

    /// 内包する記述子群のイテレーターを返す。
    #[inline]
    pub fn iter(&self) -> DescriptorIter<'a> {
        DescriptorIter(self.0)
    }

    /// 内包する記述子群から`T`のタグと一致する記述子を読み取って返す。
    ///
    /// `T`のタグと一致する記述子がない場合は`None`を返す。
    pub fn get<T: Descriptor<'a>>(&self) -> Option<T> {
        self.iter()
            .find(|d| d.tag == T::TAG)
            .and_then(|d| T::read(d.data))
    }

    /// 内包する記述子群から`T`のタグと一致する記述子をすべて読み取って返す。
    pub fn get_all<T: Descriptor<'a>>(&self) -> impl Iterator<Item = T> + 'a {
        self.iter().filter_map(|d| {
            if d.tag == T::TAG {
                T::read(d.data)
            } else {
                None
            }
        })
    }
}

impl<'a> fmt::Debug for DescriptorBlock<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("DescriptorBlock(")?;
        f.debug_list().entries(self).finish()?;
        f.write_str(")")
    }
}

impl<'a> IntoIterator for &DescriptorBlock<'a> {
    type Item = RawDescriptor<'a>;
    type IntoIter = DescriptorIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// [`DescriptorBlock`]のイテレーター。
#[derive(Clone)]
pub struct DescriptorIter<'a>(&'a [u8]);

impl<'a> Iterator for DescriptorIter<'a> {
    type Item = RawDescriptor<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let [tag, length, ref rem @ ..] = *self.0 else {
            return None;
        };
        let Some((data, tail)) = rem.split_at_checked(length as usize) else {
            return None;
        };

        self.0 = tail;
        Some(RawDescriptor { tag, data })
    }
}

impl<'a> std::iter::FusedIterator for DescriptorIter<'a> {}

impl<'a> fmt::Debug for DescriptorIter<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DescriptorIter(")?;
        f.debug_list().entries(self.clone()).finish()?;
        f.write_str(")")
    }
}
