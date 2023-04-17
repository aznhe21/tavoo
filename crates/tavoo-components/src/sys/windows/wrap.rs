use windows::core::{Result, PWSTR};
use windows::Win32::Foundation::{BOOL, FALSE};

/// [`wrap`]等で引数として使われる、Windows側の型。
pub trait WrappedValue {
    /// `wrap`等から返されるRust側の型。
    type Native;

    /// 関数に渡される初期値。
    fn placeholder() -> Self;

    /// Windows側の型からRust側の型に変換する。
    fn into_native(self) -> Self::Native;
}

impl WrappedValue for BOOL {
    type Native = bool;

    #[inline]
    fn placeholder() -> Self {
        FALSE
    }

    #[inline]
    fn into_native(self) -> Self::Native {
        self.as_bool()
    }
}

impl WrappedValue for PWSTR {
    type Native = super::com::CoString;

    #[inline]
    fn placeholder() -> Self {
        PWSTR::null()
    }

    #[inline]
    fn into_native(self) -> Self::Native {
        unsafe { super::com::CoString::from_ptr(self) }
    }
}

impl<T> WrappedValue for Option<T> {
    type Native = Option<T>;

    #[inline]
    fn placeholder() -> Self {
        None
    }

    #[inline]
    fn into_native(self) -> Self::Native {
        self
    }
}

/// 戻り値を引数として渡す種類のCOM関数を簡便に扱うための関数。
///
/// COM関数を呼び出すクロージャを指定する。
/// クロージャにはCOM関数に指定するための型が渡され、
/// その値は戻り値にする際にRust側の型に変換される。
///
/// # サンプル
///
/// ```ignore
/// extern "C" {
///     fn SomeComFunction(arg: *mut BOOL) -> Result<()>;
/// }
/// // `retval`は`bool`となる
/// let retval = wrap(|a| SomeComFunction(a))?;
/// ```
pub fn wrap<T, F>(f: F) -> Result<T::Native>
where
    T: WrappedValue,
    F: FnOnce(*mut T) -> Result<()>,
{
    let mut v1 = T::placeholder();
    f(&mut v1)?;
    Ok(v1.into_native())
}

/// [`wrap`]と同じだが二値を受け取るCOM関数に使える。
///
/// # サンプル
///
/// ```ignore
/// extern "C" {
///     fn SomeComFunction(arg1: *mut BOOL, arg2: *mut Option<IUnknown>) -> Result<()>;
/// }
/// // `rv1`は`bool`、`rv2`は`Option<IUnknown>`となる
/// let (rv1, rv2) = wrap(|a, b| SomeComFunction(a, b))?;
/// ```
pub fn wrap2<T1, T2, F>(f: F) -> Result<(T1::Native, T2::Native)>
where
    T1: WrappedValue,
    T2: WrappedValue,
    F: FnOnce(*mut T1, *mut T2) -> Result<()>,
{
    let mut v1 = T1::placeholder();
    let mut v2 = T2::placeholder();
    f(&mut v1, &mut v2)?;
    Ok((v1.into_native(), v2.into_native()))
}
