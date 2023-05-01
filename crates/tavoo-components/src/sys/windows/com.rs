#![allow(dead_code)]

use std::fmt;
use std::mem::{ManuallyDrop, MaybeUninit};
use std::ops;
use std::ptr::NonNull;

use windows::core::{self as C, Result as WinResult, PCWSTR, PWSTR};
use windows::Win32::Foundation as F;
use windows::Win32::System::Com;

#[derive(Default, Clone)]
pub struct RawPropVariant(pub Com::StructuredStorage::PROPVARIANT);

impl From<Com::StructuredStorage::PROPVARIANT> for RawPropVariant {
    fn from(value: Com::StructuredStorage::PROPVARIANT) -> RawPropVariant {
        RawPropVariant(value)
    }
}

impl Drop for RawPropVariant {
    fn drop(&mut self) {
        let _ = unsafe { Com::StructuredStorage::PropVariantClear(&mut self.0) };
    }
}

// 全部入れるのは面倒なので使いそうなやつだけ入れておく
#[derive(Debug, Default, Clone, PartialEq)]
pub enum PropVariant {
    #[default]
    Empty,
    I8(i8),
    U8(u8),
    I16(i16),
    U16(u16),
    I32(i32),
    U32(u32),
    I64(i64),
    U64(u64),
    F32(f32),
    F64(f64),
    Bool(bool),
    IUnknown(C::IUnknown),
}

impl PropVariant {
    pub fn new(value: &Com::StructuredStorage::PROPVARIANT) -> Option<PropVariant> {
        unsafe {
            let v = &value.Anonymous.Anonymous;
            match v.vt {
                Com::VT_EMPTY => Some(PropVariant::Empty),
                Com::VT_I1 => Some(PropVariant::I8(v.Anonymous.cVal as i8)),
                Com::VT_UI1 => Some(PropVariant::U8(v.Anonymous.bVal)),
                Com::VT_I2 => Some(PropVariant::I16(v.Anonymous.iVal)),
                Com::VT_UI2 => Some(PropVariant::U16(v.Anonymous.uiVal)),
                Com::VT_I4 => Some(PropVariant::I32(v.Anonymous.intVal)),
                Com::VT_UI4 => Some(PropVariant::U32(v.Anonymous.uintVal)),
                Com::VT_I8 => Some(PropVariant::I64(v.Anonymous.hVal)),
                Com::VT_UI8 => Some(PropVariant::U64(v.Anonymous.uhVal)),
                Com::VT_R4 => Some(PropVariant::F32(v.Anonymous.fltVal)),
                Com::VT_R8 => Some(PropVariant::F64(v.Anonymous.dblVal)),
                Com::VT_BOOL => Some(PropVariant::Bool(v.Anonymous.boolVal.as_bool())),
                Com::VT_UNKNOWN => Some(PropVariant::IUnknown(
                    v.Anonymous.punkVal.as_ref().unwrap().clone(),
                )),
                _ => None,
            }
        }
    }

    pub fn to_raw(&self) -> Com::StructuredStorage::PROPVARIANT {
        use Com::StructuredStorage::PROPVARIANT_0_0_0 as V;

        let (vt, val) = match *self {
            PropVariant::Empty => (Com::VT_EMPTY, V::default()),
            PropVariant::I8(v) => (Com::VT_I1, V { cVal: v as u8 }),
            PropVariant::U8(v) => (Com::VT_UI1, V { bVal: v }),
            PropVariant::I16(v) => (Com::VT_I2, V { iVal: v }),
            PropVariant::U16(v) => (Com::VT_UI2, V { uiVal: v }),
            PropVariant::I32(v) => (Com::VT_I4, V { intVal: v }),
            PropVariant::U32(v) => (Com::VT_UI4, V { uintVal: v }),
            PropVariant::I64(v) => (Com::VT_I8, V { hVal: v }),
            PropVariant::U64(v) => (Com::VT_UI8, V { uhVal: v }),
            PropVariant::F32(v) => (Com::VT_R4, V { fltVal: v }),
            PropVariant::F64(v) => (Com::VT_R8, V { dblVal: v }),
            PropVariant::Bool(v) => (Com::VT_BOOL, V { boolVal: v.into() }),
            PropVariant::IUnknown(ref v) => unsafe {
                // ManuallyDropなのにWeakじゃなくてリークしてしまうので、
                // transmute_copyによりAddRefを回避
                let v: C::IUnknown = std::mem::transmute_copy(v);
                (
                    Com::VT_UNKNOWN,
                    V {
                        punkVal: ManuallyDrop::new(Some(v)),
                    },
                )
            },
        };
        Com::StructuredStorage::PROPVARIANT {
            Anonymous: Com::StructuredStorage::PROPVARIANT_0 {
                Anonymous: std::mem::ManuallyDrop::new(Com::StructuredStorage::PROPVARIANT_0_0 {
                    vt,
                    Anonymous: val,
                    ..Default::default()
                }),
            },
        }
    }
}

impl TryFrom<Com::StructuredStorage::PROPVARIANT> for PropVariant {
    type Error = TryFromPropVariantError;

    fn try_from(
        mut value: Com::StructuredStorage::PROPVARIANT,
    ) -> Result<PropVariant, TryFromPropVariantError> {
        unsafe {
            let v = &mut value.Anonymous.Anonymous;
            match v.vt {
                Com::VT_EMPTY => Ok(PropVariant::Empty),
                Com::VT_I1 => Ok(PropVariant::I8(v.Anonymous.cVal as i8)),
                Com::VT_UI1 => Ok(PropVariant::U8(v.Anonymous.bVal)),
                Com::VT_I2 => Ok(PropVariant::I16(v.Anonymous.iVal)),
                Com::VT_UI2 => Ok(PropVariant::U16(v.Anonymous.uiVal)),
                Com::VT_I4 => Ok(PropVariant::I32(v.Anonymous.intVal)),
                Com::VT_UI4 => Ok(PropVariant::U32(v.Anonymous.uintVal)),
                Com::VT_I8 => Ok(PropVariant::I64(v.Anonymous.hVal)),
                Com::VT_UI8 => Ok(PropVariant::U64(v.Anonymous.uhVal)),
                Com::VT_R4 => Ok(PropVariant::F32(v.Anonymous.fltVal)),
                Com::VT_R8 => Ok(PropVariant::F64(v.Anonymous.dblVal)),
                Com::VT_BOOL => Ok(PropVariant::Bool(v.Anonymous.boolVal.as_bool())),
                Com::VT_UNKNOWN => Ok(PropVariant::IUnknown(
                    ManuallyDrop::take(&mut v.Anonymous.punkVal).unwrap(),
                )),
                _ => Err(TryFromPropVariantError(value.into())),
            }
        }
    }
}

pub struct TryFromPropVariantError(pub RawPropVariant);

impl fmt::Display for TryFromPropVariantError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("unknown PROPVARIANT type")
    }
}

impl fmt::Debug for TryFromPropVariantError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("TryFromPropVariantError").finish()
    }
}

impl std::error::Error for TryFromPropVariantError {}

/// `CoTaskMemAlloc`・`CoTaskMemFree`によってメモリが管理されるオブジェクト。
pub struct CoBox<T: ?Sized>(NonNull<T>);

impl<T: ?Sized> CoBox<T> {
    #[inline]
    pub fn into_raw(b: CoBox<T>) -> *mut T {
        let ptr = b.0.as_ptr();
        std::mem::forget(b);
        ptr
    }

    #[inline]
    pub unsafe fn from_raw(raw: *mut T) -> CoBox<T> {
        unsafe { CoBox(NonNull::new_unchecked(raw)) }
    }
}

impl<T> CoBox<T> {
    pub fn new(val: T) -> WinResult<CoBox<T>> {
        let mut b = CoBox::try_new_uninit()?;
        b.write(val);
        // Safety: 書き込み済み
        unsafe { Ok(b.assume_init()) }
    }

    pub fn try_new_uninit() -> WinResult<CoBox<MaybeUninit<T>>> {
        let ptr = unsafe { Com::CoTaskMemAlloc(std::mem::size_of::<T>()).cast::<MaybeUninit<T>>() };
        NonNull::new(ptr)
            .map(CoBox)
            .ok_or_else(C::Error::from_win32)
    }
}

impl<T> CoBox<[T]> {
    pub fn try_new_uninit_slice(len: usize) -> WinResult<CoBox<[MaybeUninit<T>]>> {
        if len == 0 {
            return Err(F::E_INVALIDARG.into());
        }

        let ptr =
            unsafe { Com::CoTaskMemAlloc(std::mem::size_of::<T>() * len).cast::<MaybeUninit<T>>() };
        if ptr.is_null() {
            Err(C::Error::from_win32())
        } else {
            let ptr = std::ptr::slice_from_raw_parts_mut(ptr, len);
            Ok(CoBox(unsafe { NonNull::new_unchecked(ptr) }))
        }
    }
}

impl<T> CoBox<MaybeUninit<T>> {
    pub unsafe fn assume_init(self) -> CoBox<T> {
        let raw = CoBox::into_raw(self);
        unsafe { CoBox::from_raw(raw as *mut T) }
    }
}

impl<T> CoBox<[MaybeUninit<T>]> {
    pub unsafe fn assume_init(self) -> CoBox<[T]> {
        let raw = CoBox::into_raw(self);
        unsafe { CoBox::from_raw(raw as *mut [T]) }
    }
}

impl<T: ?Sized> ops::Deref for CoBox<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe { self.0.as_ref() }
    }
}

impl<T: ?Sized> ops::DerefMut for CoBox<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        unsafe { self.0.as_mut() }
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for CoBox<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for CoBox<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl<T: ?Sized> Drop for CoBox<T> {
    fn drop(&mut self) {
        unsafe { Com::CoTaskMemFree(Some(self.0.as_ptr().cast_const().cast())) };
    }
}

/// `CoTaskMemAlloc`・`CoTaskMemFree`によってメモリが管理される文字列。
pub struct CoString(Option<CoBox<[u16]>>);

impl CoString {
    #[inline]
    pub const fn new() -> CoString {
        CoString(None)
    }

    pub unsafe fn from_ptr(ptr: PWSTR) -> CoString {
        if ptr.is_null() {
            CoString(None)
        } else {
            let len = unsafe { ptr.as_wide() }.len();
            let slice = std::ptr::slice_from_raw_parts_mut(ptr.0, len);
            CoString(Some(unsafe { CoBox::from_raw(slice) }))
        }
    }

    pub fn from_str(s: &str) -> WinResult<CoString> {
        let len = s.encode_utf16().count();
        if len == 0 {
            return Ok(CoString(None));
        }

        let mut array = CoBox::try_new_uninit_slice(len + 1)?;
        for (dst, src) in array.iter_mut().zip(s.encode_utf16()) {
            dst.write(src);
        }
        array[len].write(0);

        // Safety: 全要素書き込み済み
        let array = unsafe { array.assume_init() };
        Ok(CoString(Some(array)))
    }

    pub fn to_pwstr(&mut self) -> PWSTR {
        match &mut self.0 {
            None => PWSTR::null(),
            Some(array) => PWSTR(array.as_mut_ptr()),
        }
    }

    pub fn to_pcwstr(&self) -> PCWSTR {
        match &self.0 {
            None => PCWSTR::null(),
            Some(array) => PCWSTR(array.as_ptr()),
        }
    }

    pub fn to_string(&self) -> Result<String, std::string::FromUtf16Error> {
        match &self.0 {
            None => Ok(String::new()),
            Some(array) => String::from_utf16(&**array),
        }
    }

    pub fn into_pwstr(self) -> PWSTR {
        match self.0 {
            None => PWSTR::null(),
            Some(array) => PWSTR(CoBox::into_raw(array).cast()),
        }
    }
}

/// `s`を`CoTaskMemAlloc`を使って`PWSTR`に変換する。
///
/// 戻り値を解放する責任は呼び出し側にある。
#[inline]
pub fn to_pwstr(s: &str) -> WinResult<PWSTR> {
    CoString::from_str(s).map(|s| s.into_pwstr())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_pwstr() {
        unsafe {
            assert_eq!(to_pwstr(""), Ok(PWSTR::null()));
            assert_eq!(
                to_pwstr("hoge").unwrap().as_wide(),
                "hoge".encode_utf16().collect::<Vec<_>>()
            );
        }
    }
}
