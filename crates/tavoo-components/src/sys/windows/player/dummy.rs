use windows::core::{self as C, implement};
use windows::Win32::Foundation as F;
use windows::Win32::Media::MediaFoundation as MF;

use super::utils::WinResult;

#[implement(MF::IMFMediaStream)]
pub struct DummyStream;

#[allow(non_snake_case)]
impl MF::IMFMediaEventGenerator_Impl for DummyStream {
    fn GetEvent(
        &self,
        _: MF::MEDIA_EVENT_GENERATOR_GET_EVENT_FLAGS,
    ) -> WinResult<MF::IMFMediaEvent> {
        Err(F::E_NOTIMPL.into())
    }

    fn BeginGetEvent(
        &self,
        _: Option<&MF::IMFAsyncCallback>,
        _: Option<&C::IUnknown>,
    ) -> WinResult<()> {
        Err(F::E_NOTIMPL.into())
    }

    fn EndGetEvent(&self, _: Option<&MF::IMFAsyncResult>) -> WinResult<MF::IMFMediaEvent> {
        Err(F::E_NOTIMPL.into())
    }

    fn QueueEvent(
        &self,
        _: u32,
        _: *const C::GUID,
        _: C::HRESULT,
        _: *const windows::Win32::System::Com::StructuredStorage::PROPVARIANT,
    ) -> WinResult<()> {
        Err(F::E_NOTIMPL.into())
    }
}

#[allow(non_snake_case)]
impl MF::IMFMediaStream_Impl for DummyStream {
    fn GetMediaSource(&self) -> WinResult<MF::IMFMediaSource> {
        Err(F::E_NOTIMPL.into())
    }

    fn GetStreamDescriptor(&self) -> WinResult<MF::IMFStreamDescriptor> {
        Err(F::E_NOTIMPL.into())
    }

    fn RequestSample(&self, _: Option<&C::IUnknown>) -> WinResult<()> {
        Err(F::E_NOTIMPL.into())
    }
}
