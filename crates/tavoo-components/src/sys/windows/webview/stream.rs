use std::cell::UnsafeCell;
use std::ffi::c_void;
use std::io::{self, Read, Write};

use windows::core::{implement, Result as WinResult, HRESULT};
use windows::Win32::Foundation as F;
use windows::Win32::System::Com::{
    self as Com, ISequentialStream_Impl, IStream, IStream_Impl, STGC_DEFAULT,
};

/// `IStream`を使って[`Read`]や[`Write`]をするためのラッパー型。
#[derive(Debug, Clone)]
pub struct DerivedStream(pub IStream);

impl Read for DerivedStream {
    /// `buf`が`u32`を超える容量の場合、読み取られる容量は`u32::MAX`に制限される。
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let len = buf.len().try_into().unwrap_or(u32::MAX);
        let mut read = 0;
        match unsafe { self.0.Read(buf.as_mut_ptr().cast(), len, Some(&mut read)) } {
            // 32ビット未満はサポートしないので`as`で良い
            F::S_OK | F::S_FALSE => Ok(read as usize),
            hr => Err(crate::sys::error::hr_to_io(hr)),
        }
    }
}

impl Write for DerivedStream {
    /// `buf`が`u32`を超える容量の場合、書き込まれる容量は`u32::MAX`に制限される。
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let len = buf.len().try_into().unwrap_or(u32::MAX);
        let mut written = 0;
        match unsafe { self.0.Write(buf.as_ptr().cast(), len, Some(&mut written)) } {
            // 32ビット未満はサポートしないので`as`で良い
            F::S_OK => Ok(written as usize),
            hr => Err(crate::sys::error::hr_to_io(hr)),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match unsafe { self.0.Commit(STGC_DEFAULT) } {
            Ok(()) => Ok(()),
            Err(e) => Err(crate::sys::error::hr_to_io(e.code())),
        }
    }
}

/// [`Read`]を実装する型を`IStream`として扱うためのラッパー型。
#[implement(IStream)]
pub struct ReadStream(UnsafeCell<Box<dyn Read>>);

impl ReadStream {
    #[inline]
    pub fn from_read<R: Read + 'static>(read: R) -> ReadStream {
        ReadStream(UnsafeCell::new(Box::new(read)))
    }

    #[inline]
    pub const fn from_boxed(read: Box<dyn Read>) -> ReadStream {
        ReadStream(UnsafeCell::new(read))
    }
}

#[allow(non_snake_case)]
impl ISequentialStream_Impl for ReadStream {
    fn Read(&self, pv: *mut c_void, cb: u32, pcbread: *mut u32) -> HRESULT {
        let pcbread = unsafe { pcbread.as_mut() };

        // 32ビット未満はサポートしないので`as`で良い
        let cb = cb as usize;
        let r = unsafe {
            let buf = std::slice::from_raw_parts_mut(pv.cast(), cb);
            (*self.0.get()).read(buf)
        };
        match r {
            Ok(read) => {
                if let Some(pcbread) = pcbread {
                    *pcbread = read as u32;
                }
                if read < cb {
                    F::S_FALSE
                } else {
                    F::S_OK
                }
            }
            Err(e) => crate::sys::error::io_to_hr(e),
        }
    }

    fn Write(&self, _: *const c_void, _: u32, _: *mut u32) -> HRESULT {
        F::E_NOTIMPL
    }
}

#[allow(non_snake_case)]
impl IStream_Impl for ReadStream {
    fn Seek(&self, _: i64, _: Com::STREAM_SEEK, _: *mut u64) -> WinResult<()> {
        Err(F::E_NOTIMPL.into())
    }

    fn SetSize(&self, _: u64) -> WinResult<()> {
        Err(F::E_NOTIMPL.into())
    }

    fn CopyTo(
        &self,
        pstm: Option<&IStream>,
        cb: u64,
        pcbread: *mut u64,
        pcbwritten: *mut u64,
    ) -> WinResult<()> {
        let pstm = pstm.ok_or(F::E_POINTER)?;
        let pcbread = unsafe { pcbread.as_mut() };
        let pcbwritten = unsafe { pcbwritten.as_mut() };
        let reader = unsafe { &mut *self.0.get() };

        let mut writer = DerivedStream(pstm.clone());
        match std::io::copy(&mut reader.take(cb), &mut writer) {
            Ok(written) => {
                if let Some(pcbread) = pcbread {
                    *pcbread = written;
                }
                if let Some(pcbwritten) = pcbwritten {
                    *pcbwritten = written;
                }
                Ok(())
            }
            Err(e) => Err(crate::sys::error::io_to_hr(e).into()),
        }
    }

    fn Commit(&self, _: Com::STGC) -> WinResult<()> {
        Err(F::E_NOTIMPL.into())
    }

    fn Revert(&self) -> WinResult<()> {
        Err(F::E_NOTIMPL.into())
    }

    fn LockRegion(&self, _: u64, _: u64, _: Com::LOCKTYPE) -> WinResult<()> {
        Err(F::E_NOTIMPL.into())
    }

    fn UnlockRegion(&self, _: u64, _: u64, _: u32) -> WinResult<()> {
        Err(F::E_NOTIMPL.into())
    }

    fn Stat(&self, _: *mut Com::STATSTG, _: Com::STATFLAG) -> WinResult<()> {
        Err(F::E_NOTIMPL.into())
    }

    fn Clone(&self) -> WinResult<IStream> {
        Err(F::E_NOTIMPL.into())
    }
}
