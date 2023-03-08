use std::fmt;
use std::mem::ManuallyDrop;

use windows::core as C;
use windows::Win32::Foundation as F;
use windows::Win32::Media::MediaFoundation as MF;
use windows::Win32::System::Com;

pub type WinResult<T> = windows::core::Result<T>;

pub unsafe fn get_stream_descriptor_by_index(
    pd: &MF::IMFPresentationDescriptor,
    index: u32,
) -> WinResult<(bool, MF::IMFStreamDescriptor)> {
    let mut selected = false.into();
    let mut sd = None;
    pd.GetStreamDescriptorByIndex(index, &mut selected, &mut sd)?;
    Ok((selected.as_bool(), sd.unwrap()))
}

#[derive(Default, Clone)]
pub struct RawPropVariant(pub Com::StructuredStorage::PROPVARIANT);

impl From<Com::StructuredStorage::PROPVARIANT> for RawPropVariant {
    fn from(value: Com::StructuredStorage::PROPVARIANT) -> RawPropVariant {
        RawPropVariant(value)
    }
}

impl Drop for RawPropVariant {
    fn drop(&mut self) {
        unsafe {
            let _ = Com::StructuredStorage::PropVariantClear(&mut self.0);
        }
    }
}

// 全部入れるのは面倒なので使いそうなやつだけ入れておく
#[allow(dead_code)]
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
                Com::VT_I1 => Some(PropVariant::I8(v.Anonymous.cVal.0 as i8)),
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
            PropVariant::I8(v) => (
                Com::VT_I1,
                V {
                    cVal: F::CHAR(v as u8),
                },
            ),
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
                Com::VT_I1 => Ok(PropVariant::I8(v.Anonymous.cVal.0 as i8)),
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
