macro_rules! impl_id {
    ($name:ident) => {
        impl $name {
            #[doc = concat!("`n`がゼロでなければ`", stringify!($name), "`を生成する。")]
            #[inline]
            pub fn new(n: u16) -> Option<$name> {
                NonZeroU16::new(n).map($name)
            }

            /// プリミティブ型として値を返す。
            #[inline]
            pub fn get(self) -> u16 {
                self.0.get()
            }
        }

        crate::utils::delegate_fmt!($name);
    };
}
