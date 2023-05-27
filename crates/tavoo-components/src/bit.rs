//! ビット列を読み取る。

use std::marker::PhantomData;

mod sealed {
    use std::ops;

    pub trait ReadBe: ops::Shl<usize, Output = Self> + ops::Shr<u32, Output = Self> {
        /// この型のビット数。
        const BITS: u32;
        /// この型のバイト数。
        const SIZE: usize;

        /// `bytes`から値を読み取る。
        unsafe fn from_be_byte_ptr(bytes: *const u8) -> Self;
    }

    pub trait ReadBits<const N: u32> {
        /// `N`ビットを読み取るのに必要な大きさを備える型。
        type Output: ReadBe;
    }

    pub trait ReadInsideBits<const N: u32> {
        type Output: ReadBe;
    }

    macro_rules! impl_read_be {
        ($($t:ident),*) => {
            $(
                impl ReadBe for $t {
                    const BITS: u32 = $t::BITS;
                    const SIZE: usize = std::mem::size_of::<$t>();

                    unsafe fn from_be_byte_ptr(bytes: *const u8) -> $t {
                        <$t>::from_be_bytes(unsafe { *(bytes as *const _) })
                    }
                }
            )*
        };
    }
    macro_rules! impl_read_bits {
        ($($t:ident for ($($n:literal),*);)*) => {
            $($(
                impl ReadBits<$n> for () {
                    type Output = $t;
                }
            )*)*
        };
    }
    macro_rules! impl_read_inside_bits {
        ($($t:ident for ($($n:literal),*);)*) => {
            $($(
                impl ReadInsideBits<$n> for () {
                    type Output = $t;
                }
            )*)*
        };
    }

    impl_read_be!(u8, u16, u32);
    impl_read_bits!(
        u8 for (1);
        u16 for (2, 3, 4, 5, 6, 7, 8, 9);
        u32 for (10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25);
    );
    impl_read_inside_bits!(
        u8 for (1, 2, 3, 4, 5, 6, 7, 8);
        u16 for (9, 10, 11, 12, 13, 14, 15, 16);
        u32 for (17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32);
    );
}

/// ビット列をビッグエンディアンで読み取るためのオブジェクト。
#[derive(Debug)]
pub struct BitReader<'a> {
    buffer: *const u8,
    len: usize,
    nbits: usize,
    index: usize,
    _marker: PhantomData<&'a [u8]>,
}

impl<'a> BitReader<'a> {
    /// `buffer`をビット単位で読み取るための`BitReader`を生成する。
    #[inline]
    pub fn new(buffer: &'a [u8]) -> BitReader<'a> {
        BitReader {
            buffer: buffer.as_ptr(),
            len: buffer.len(),
            nbits: buffer.len() << 3,
            index: 0,
            _marker: PhantomData,
        }
    }

    /// 残りのビット数を返す。
    ///
    /// # サンプル
    ///
    /// ```
    /// use tavoo_components::bit::BitReader;
    ///
    /// let mut br = BitReader::new(&[0, 1, 2]);
    /// assert_eq!(br.bits(), 24);
    /// br.skip(12);
    /// assert_eq!(br.bits(), 12);
    /// br.skip(100);
    /// assert_eq!(br.bits(), 0);
    /// ```
    #[inline]
    pub const fn bits(&self) -> usize {
        self.nbits - self.index
    }

    /// 読み取り途中の要素を含めた残りの長さをバイト単位で返す。
    ///
    /// # サンプル
    ///
    /// ```
    /// use tavoo_components::bit::BitReader;
    ///
    /// let mut br = BitReader::new(&[0, 1, 2]);
    /// assert_eq!(br.len(), 3);
    /// br.skip(12);
    /// assert_eq!(br.len(), 2);
    /// br.skip(100);
    /// assert_eq!(br.len(), 0);
    /// ```
    #[inline]
    pub const fn len(&self) -> usize {
        self.len - (self.index >> 3)
    }

    /// 読み取れるビットがない場合に`true`を返す。
    ///
    /// # サンプル
    ///
    /// ```
    /// use tavoo_components::bit::BitReader;
    ///
    /// let mut br = BitReader::new(&[0, 1, 2]);
    /// assert!(!br.is_empty());
    /// br.skip(23);
    /// assert!(!br.is_empty());
    /// br.skip(1);
    /// assert!(br.is_empty());
    /// ```
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.nbits <= self.index
    }

    /// 読み取り途中の要素を含めた残りのバイト列を返す。
    ///
    /// # サンプル
    ///
    /// ```
    /// use tavoo_components::bit::BitReader;
    ///
    /// let mut br = BitReader::new(&[0, 1, 2]);
    /// assert_eq!(br.as_bytes(), &[0, 1, 2]);
    /// br.skip(7);
    /// assert_eq!(br.as_bytes(), &[0, 1, 2]);
    /// br.skip(1);
    /// assert_eq!(br.as_bytes(), &[1, 2]);
    /// ```
    #[inline]
    pub const fn as_bytes(&self) -> &'a [u8] {
        unsafe { std::slice::from_raw_parts(self.buffer.add(self.index >> 3), self.len()) }
    }

    /// `bits`で指定されたビット数の分だけ読み取りを飛ばす。
    ///
    /// # サンプル
    ///
    /// ```
    /// use tavoo_components::bit::BitReader;
    ///
    /// let mut br = BitReader::new(&[0, 1]);
    /// assert_eq!(br.bits(), 16);
    /// br.skip(12);
    /// assert_eq!(br.bits(), 4);
    /// br.skip(8);
    /// assert_eq!(br.bits(), 0);
    /// ```
    #[inline]
    pub fn skip(&mut self, bits: u8) {
        self.index = std::cmp::min(self.nbits, self.index + bits as usize);
    }

    /// 範囲チェックをせずに次のビットを読み取る。
    ///
    /// # 安全性
    ///
    /// 範囲チェックは行われないため、範囲外にアクセスした際の動作は未定義である。
    #[inline]
    pub unsafe fn read1_unchecked(&mut self) -> bool {
        let n = unsafe { *self.buffer.add(self.index >> 3) };
        let v = n << (self.index & 7) >> 7 != 0;

        if self.index < self.nbits {
            self.index += 1;
        }
        v
    }

    /// 次のビットを読み取る。
    ///
    /// 既に範囲外の場合は`None`を返す。
    ///
    /// # サンプル
    ///
    /// ```
    /// use tavoo_components::bit::BitReader;
    ///
    /// let mut br = BitReader::new(&[0b10101010]);
    /// assert_eq!(br.read1(), Some(true));
    /// assert_eq!(br.read1(), Some(false));
    /// assert_eq!(br.read1(), Some(true));
    /// assert_eq!(br.read1(), Some(false));
    /// assert_eq!(br.read1(), Some(true));
    /// assert_eq!(br.read1(), Some(false));
    /// assert_eq!(br.read1(), Some(true));
    /// assert_eq!(br.read1(), Some(false));
    /// assert_eq!(br.read1(), None);
    /// assert_eq!(br.read1(), None);
    /// ```
    #[inline]
    pub fn read1(&mut self) -> Option<bool> {
        if self.len <= self.index >> 3 {
            None
        } else {
            Some(unsafe { self.read1_unchecked() })
        }
    }

    #[inline]
    unsafe fn read_inner<T: sealed::ReadBe>(&mut self, n: u32) -> T {
        let t = unsafe { T::from_be_byte_ptr(self.buffer.add(self.index >> 3)) };
        let v = t << (self.index & 7) >> (T::BITS - n);
        self.index = std::cmp::min(self.nbits, self.index + n as usize);
        v
    }

    /// 範囲チェックをせずに`N`で指定されたビット数分のビットをビッグエンディアンで読み取る。
    /// メモリから読み取る際の単位は戻り値の型であり、この型は`(N+7).next_power_of_two()`ビットの型である
    /// （例えば`N`が`1`では`u8`、`2..=9`では`u16`）。
    ///
    /// # 安全性
    ///
    /// 範囲チェックは行われないため、範囲外にアクセスした際の動作は未定義である。
    #[inline]
    pub unsafe fn read_unchecked<const N: u32>(&mut self) -> <() as sealed::ReadBits<N>>::Output
    where
        (): sealed::ReadBits<N>,
    {
        unsafe { self.read_inner(N) }
    }

    /// `N`で指定されたビット数分のビットをビッグエンディアンで読み取る。
    /// メモリから読み取る際の単位は`N`ではなく戻り値型の大きさであり、
    /// この型は`(N+7).next_power_of_two()`ビットの型である（例えば`N`が`1`では`u8`、`2..=9`では`u16`）。
    ///
    /// # サンプル
    ///
    /// ```
    /// use tavoo_components::bit::BitReader;
    ///
    /// let mut br = BitReader::new(&[0b101000_01, 0b0100_0001, 0b0000_0001, 0x12, 0x34, 0x56, 0x78]);
    /// br.skip(6);
    /// assert_eq!(br.read::<6>(), Some(0b010100_u16));
    /// assert_eq!(br.read::<12>(), Some(0b0001_0000_0001_u32));
    /// assert_eq!(br.read::<24>(), Some(0x123456_u32));
    ///
    /// // まだ8ビット分読み取れるがアクセス単位が`u16`であるため範囲外
    /// assert_eq!(br.bits(), 8);
    /// assert_eq!(br.read::<8>(), None::<u16>);
    /// ```
    #[inline]
    pub fn read<const N: u32>(&mut self) -> Option<<() as sealed::ReadBits<N>>::Output>
    where
        (): sealed::ReadBits<N>,
    {
        if self.len() < <<() as sealed::ReadBits<N>>::Output as sealed::ReadBe>::SIZE {
            None
        } else {
            Some(unsafe { self.read_inner(N) })
        }
    }

    /// バイト境界を跨がず、`N`で指定されたビット数分のビットをビッグエンディアンで読み取る。
    ///
    /// 現在位置によっては`N`ビットを読み取るのに必要なバイト数を確保できないために
    /// 間違った値が返される場合がある。
    ///
    /// # サンプル
    ///
    /// ```
    /// use tavoo_components::bit::BitReader;
    ///
    /// let mut br = BitReader::new(&[0b000000_11, 0b11_000000]);
    /// br.skip(6);
    /// // 6ビット目から4ビット読み取ると0b1111になるはずだが、
    /// // 1バイト目しか読み取れないため間違った値になる
    /// assert_eq!(unsafe { br.read_inside_unchecked::<4>() }, 0b1100);
    /// ```
    ///
    /// # 安全性
    ///
    /// 範囲チェックは行われないため、範囲外にアクセスした際の動作は未定義である。
    pub unsafe fn read_inside_unchecked<const N: u32>(
        &mut self,
    ) -> <() as sealed::ReadInsideBits<N>>::Output
    where
        (): sealed::ReadInsideBits<N>,
    {
        unsafe { self.read_inner(N) }
    }

    /// バイト境界を跨がず、`N`で指定されたビット数分のビットをビッグエンディアンで読み取る。
    ///
    /// バイト境界を跨ぐ場合は`None`を返す。
    ///
    /// [`read`][BitReader::read]とは異なり、戻り値の型は`N`を格納できる最小限の大きさである
    /// （例えば`N`が`1..=8`では`u8`、`9..=16`では`u16`）。
    ///
    /// # サンプル
    ///
    /// ```
    /// use tavoo_components::bit::BitReader;
    ///
    /// let mut br = BitReader::new(&[0b101000_01, 0b01010101, 0b11111111, 0b11111111, 0b111111_00, 1, 2]);
    /// assert_eq!(br.read_inside::<6>(), Some(0b101000_u8));
    /// assert_eq!(br.read_inside::<10>(), Some(0b01_01010101));
    /// assert_eq!(br.read_inside::<22>(), Some(0b11111111_11111111_111111));
    ///
    /// // 17ビットを格納するのに32ビット必要だが18ビットしかないため読み取れない
    /// assert_eq!(br.bits(), 18);
    /// assert_eq!(br.read_inside::<17>(), None::<u32>);
    ///
    /// // 十分読み取れるビット数だが、バイト境界を跨ぐため読み取れない
    /// let mut br = BitReader::new(&[0, 0, 0, 0, 0]);
    /// br.skip(6);
    /// assert_eq!(br.bits(), 34);
    /// assert_eq!(br.read_inside::<3>(), None::<u8>);
    /// assert_eq!(br.read_inside::<11>(), None::<u16>);
    /// assert_eq!(br.read_inside::<27>(), None::<u32>);
    #[inline]
    pub fn read_inside<const N: u32>(&mut self) -> Option<<() as sealed::ReadInsideBits<N>>::Output>
    where
        (): sealed::ReadInsideBits<N>,
    {
        if self.len() < <<() as sealed::ReadInsideBits<N>>::Output as sealed::ReadBe>::SIZE
            || (self.index + N as usize)
                > (self.index >> 3 << 3)
                    + <<() as sealed::ReadInsideBits<N>>::Output as sealed::ReadBe>::BITS as usize
        {
            None
        } else {
            Some(unsafe { self.read_inner(N) })
        }
    }
}
