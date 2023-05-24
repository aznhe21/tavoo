//! ビット列を読み取る。

use std::marker::PhantomData;

mod sealed {
    use std::ops;

    pub trait ReadBits<const N: u32> {
        /// `N`ビットを読み取るのに必要な大きさを備える型。
        type Output: ops::Shl<usize, Output = Self::Output> + ops::Shr<u32, Output = Self::Output>;
        /// `Self::Output`のビット数。
        const BITS: u32;
        /// `Self::Output`のバイト数。
        const SIZE: usize;

        /// `bytes`から`Self::Output`を読み取る。
        unsafe fn from_be_byte_ptr(bytes: *const u8) -> Self::Output;
    }

    macro_rules! impl_read_bits {
        ($($t:ident for ($($n:literal),*);)*) => {
            $($(
                impl ReadBits<$n> for () {
                    type Output = $t;
                    const BITS: u32 = $t::BITS;
                    const SIZE: usize = std::mem::size_of::<$t>();

                    unsafe fn from_be_byte_ptr(bytes: *const u8) -> $t {
                        <$t>::from_be_bytes(unsafe { *(bytes as *const _) })
                    }
                }
            )*)*
        };
    }

    impl_read_bits!(
        u8 for (1);
        u16 for (2, 3, 4, 5, 6, 7, 8, 9);
        u32 for (10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25);
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
        let n = unsafe {
            <() as sealed::ReadBits<N>>::from_be_byte_ptr(self.buffer.add(self.index >> 3))
        };
        let v = n << (self.index & 7) >> (<() as sealed::ReadBits<N>>::BITS - N);
        self.index = std::cmp::min(self.nbits, self.index + N as usize);
        v
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
        if self.len() < <() as sealed::ReadBits<N>>::SIZE {
            None
        } else {
            Some(unsafe { self.read_unchecked::<N>() })
        }
    }
}
