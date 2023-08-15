use std::cell::{Cell, UnsafeCell};
use std::ffi::OsString;
use std::os::windows::prelude::OsStringExt;
use std::path::{Path, PathBuf};

use windows::core::{implement, Result as WinResult};
use windows::Win32::Foundation as F;
use windows::Win32::System::Com;
use windows::Win32::System::Memory;
use windows::Win32::System::Ole;
use windows::Win32::UI::Shell;

const FORMATETC: Com::FORMATETC = Com::FORMATETC {
    cfFormat: Ole::CF_HDROP.0,
    ptd: std::ptr::null_mut(),
    dwAspect: Com::DVASPECT_CONTENT.0,
    lindex: -1,
    tymed: Com::TYMED_HGLOBAL.0 as u32,
};

#[implement(Ole::IDropTarget)]
pub struct DropTarget {
    is_file: Cell<bool>,
    handler: UnsafeCell<Box<dyn Fn(&Path)>>,
}

impl DropTarget {
    #[inline]
    pub fn new(handler: Box<dyn Fn(&Path)>) -> DropTarget {
        DropTarget {
            is_file: Cell::new(false),
            handler: UnsafeCell::new(handler),
        }
    }
}

#[allow(non_snake_case)]
impl Ole::IDropTarget_Impl for DropTarget {
    fn DragEnter(
        &self,
        pdataobj: Option<&Com::IDataObject>,
        _: windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS,
        _: &F::POINTL,
        pdweffect: *mut Ole::DROPEFFECT,
    ) -> WinResult<()> {
        let pdataobj = pdataobj.ok_or(F::E_POINTER)?;
        let pdweffect = unsafe { pdweffect.as_mut() }.ok_or(F::E_POINTER)?;

        self.is_file
            .set(unsafe { pdataobj.QueryGetData(&FORMATETC) }.is_ok());
        *pdweffect = if self.is_file.get() {
            Ole::DROPEFFECT_COPY
        } else {
            Ole::DROPEFFECT_NONE
        };

        Ok(())
    }

    fn DragOver(
        &self,
        _: windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS,
        _: &F::POINTL,
        pdweffect: *mut Ole::DROPEFFECT,
    ) -> WinResult<()> {
        let pdweffect = unsafe { pdweffect.as_mut() }.ok_or(F::E_POINTER)?;

        *pdweffect = if self.is_file.get() {
            Ole::DROPEFFECT_COPY
        } else {
            Ole::DROPEFFECT_NONE
        };

        Ok(())
    }

    fn DragLeave(&self) -> WinResult<()> {
        Ok(())
    }

    fn Drop(
        &self,
        pdataobj: Option<&Com::IDataObject>,
        _: windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS,
        _: &F::POINTL,
        _: *mut Ole::DROPEFFECT,
    ) -> WinResult<()> {
        struct Medium(Com::STGMEDIUM);
        impl Drop for Medium {
            fn drop(&mut self) {
                unsafe {
                    Memory::GlobalUnlock(self.0.Anonymous.hGlobal);
                    Ole::ReleaseStgMedium(&mut self.0);
                }
            }
        }

        let pdataobj = pdataobj.ok_or(F::E_POINTER)?;
        let handler = unsafe { &mut *self.handler.get() };

        let medium = Medium(unsafe { pdataobj.GetData(&FORMATETC)? });
        let hdrop =
            Shell::HDROP(unsafe { Memory::GlobalLock(medium.0.Anonymous.hGlobal) } as usize as _);

        let n = unsafe { Shell::DragQueryFileW(hdrop, u32::MAX, None) };
        for i in 0..n {
            let len = unsafe { Shell::DragQueryFileW(hdrop, i, None) };

            let mut path = vec![0; len as usize + 1];
            let _actual = unsafe { Shell::DragQueryFileW(hdrop, i, Some(&mut *path)) };
            debug_assert_eq!(len, _actual);

            let path: PathBuf = OsString::from_wide(&path[..len as usize]).into();
            handler(&*path);
        }

        Ok(())
    }
}
