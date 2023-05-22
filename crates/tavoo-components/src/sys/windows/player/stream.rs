use std::collections::VecDeque;

use parking_lot::{Mutex, MutexGuard};
use windows::core::{self as C, implement, AsImpl, ComInterface, Result as WinResult};
use windows::Win32::Foundation as F;
use windows::Win32::Media::KernelStreaming::GUID_NULL;
use windows::Win32::Media::MediaFoundation as MF;

use crate::sys::com::PropVariant;

use super::source::TransportStream;

const SAMPLE_QUEUE: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Stopped,
    Paused,
    Started,
    Shutdown,
}

#[derive(Debug, Clone)]
pub struct ElementaryStream(MF::IMFMediaStream);

impl ElementaryStream {
    pub fn new(
        source: &TransportStream,
        stream_descriptor: MF::IMFStreamDescriptor,
    ) -> WinResult<ElementaryStream> {
        let event_queue = unsafe { MF::MFCreateEventQueue()? };
        let inner = Mutex::new(Inner {
            source: source.intf().downgrade().unwrap(),
            event_queue,

            stream_descriptor,

            state: State::Stopped,
            is_eos: false,

            queue: VecDeque::new(),
            requests: VecDeque::new(),
        });
        Ok(ElementaryStream(Outer { inner }.into()))
    }

    #[inline]
    pub unsafe fn from_stream(stream: MF::IMFMediaStream) -> ElementaryStream {
        ElementaryStream(stream)
    }

    #[inline]
    pub fn intf(&self) -> &MF::IMFMediaStream {
        &self.0
    }

    #[inline]
    fn inner(&self) -> parking_lot::MutexGuard<Inner> {
        let outer: &Outer = self.0.as_impl();
        outer.inner.lock()
    }

    #[inline]
    pub fn needs_data(&self) -> bool {
        self.inner().needs_data()
    }

    #[inline]
    pub fn end_of_stream(&self) -> WinResult<()> {
        Inner::end_of_stream(&mut self.inner())
    }

    #[inline]
    pub fn push_sample(&self, sample: MF::IMFSample) {
        self.inner().push_sample(sample);
    }

    #[inline]
    pub fn clear_samples(&self) {
        self.inner().clear_samples();
    }

    #[inline]
    pub fn dispatch_samples(&self) -> WinResult<()> {
        Inner::dispatch_samples(&mut self.inner())
    }

    #[inline]
    pub fn change_media_type(&self, media_type: MF::IMFMediaType) -> WinResult<()> {
        Inner::change_media_type(&mut self.inner(), media_type)
    }

    #[inline]
    pub fn start(&self, start_pos: &PropVariant) -> WinResult<()> {
        let mut inner = self.inner();
        inner.check_shutdown()?;
        Inner::start(&mut inner, start_pos)
    }

    #[inline]
    pub fn pause(&self) -> WinResult<()> {
        let mut inner = self.inner();
        inner.check_shutdown()?;
        Inner::pause(&mut inner)
    }

    #[inline]
    pub fn stop(&self) -> WinResult<()> {
        let mut inner = self.inner();
        inner.check_shutdown()?;
        Inner::stop(&mut inner)
    }

    #[inline]
    pub fn shutdown(&self) -> WinResult<()> {
        let mut inner = self.inner();
        inner.check_shutdown()?;
        Inner::shutdown(&mut inner)
    }
}

// Safety: 内包するIMFMediaStreamはOuterであり、OuterはSendであるため安全
unsafe impl Send for ElementaryStream {}

enum Message {
    Sample(MF::IMFSample),
    MediaType(MF::IMFMediaType),
}

#[implement(MF::IMFMediaStream)]
struct Outer {
    inner: Mutex<Inner>,
}

struct Inner {
    source: C::Weak<MF::IMFMediaSource>,
    event_queue: MF::IMFMediaEventQueue,

    stream_descriptor: MF::IMFStreamDescriptor,

    state: State,
    is_eos: bool,

    queue: VecDeque<Message>,
    requests: VecDeque<Option<C::IUnknown>>,
}

// Safety: C++のサンプルではスレッドをまたいで使っているので安全なはず
unsafe impl Send for Inner {}

impl Inner {
    #[inline]
    fn source_unlocked<T, F>(this: &mut MutexGuard<Self>, f: F) -> T
    where
        F: FnOnce(TransportStream) -> T,
    {
        // Safety: self.sourceはTransportStreamからdowngradeしたもの
        let ts = unsafe { TransportStream::from_source(this.source.upgrade().unwrap()) };
        MutexGuard::unlocked(this, || f(ts))
    }

    fn check_shutdown(&self) -> WinResult<()> {
        if self.state == State::Shutdown {
            Err(MF::MF_E_SHUTDOWN.into())
        } else {
            Ok(())
        }
    }

    fn needs_data(&self) -> bool {
        !self.is_eos && self.queue.len() < SAMPLE_QUEUE
    }

    fn end_of_stream(this: &mut MutexGuard<Self>) -> WinResult<()> {
        if !this.is_eos {
            this.is_eos = true;
            Inner::dispatch_samples(this)?;
        }

        Ok(())
    }

    fn push_sample(&mut self, sample: MF::IMFSample) {
        self.queue.push_back(Message::Sample(sample));
    }

    fn clear_samples(&mut self) {
        self.queue.retain(|m| !matches!(m, Message::Sample(_)));

        // 古い種別変更も消す
        while self.queue.len() >= 2 {
            let _front = self.queue.pop_front();
            debug_assert!(matches!(_front.unwrap(), Message::MediaType(_)));
        }
    }

    fn change_media_type(
        this: &mut MutexGuard<Self>,
        media_type: MF::IMFMediaType,
    ) -> WinResult<()> {
        this.queue.push_back(Message::MediaType(media_type));
        Inner::dispatch_samples(this)?;

        Ok(())
    }

    fn dispatch_samples(this: &mut MutexGuard<Self>) -> WinResult<()> {
        let r: WinResult<()> = 'r: {
            if this.state != State::Started {
                break 'r Ok(());
            }

            while !this.queue.is_empty() && !this.requests.is_empty() {
                match this.queue.pop_front().unwrap() {
                    Message::Sample(sample) => {
                        let token = this.requests.pop_front().unwrap();

                        if let Some(token) = token {
                            unsafe {
                                tri!('r, sample.SetUnknown(&MF::MFSampleExtension_Token, &token))
                            };
                        }

                        unsafe {
                            tri!('r, this.event_queue.QueueEventParamUnk(
                                MF::MEMediaSample.0 as u32,
                                &GUID_NULL,
                                F::S_OK,
                                &sample,
                            ))
                        };
                    }
                    Message::MediaType(media_type) => unsafe {
                        tri!('r, this.event_queue.QueueEventParamUnk(
                            MF::MEStreamFormatChanged.0 as u32,
                            &GUID_NULL,
                            F::S_OK,
                            &media_type,
                        ))
                    },
                }
            }

            if this.queue.is_empty() && this.is_eos {
                log::trace!("sample exhausted ({:p})", &**this);
                unsafe {
                    tri!('r, this.event_queue.QueueEventParamVar(
                        MF::MEEndOfStream.0 as u32,
                        &GUID_NULL,
                        F::S_OK,
                        std::ptr::null(),
                    ))
                };
                tri!('r, Inner::source_unlocked(this, |ts| ts.enqueue_end_of_stream()));
            } else if this.needs_data() {
                Inner::source_unlocked(this, |ts| ts.request_sample());
            }

            Ok(())
        };
        if let Err(ref e) = r {
            if this.state != State::Shutdown {
                log::debug!("error[dispatch_samples]: {}", e);
                Inner::source_unlocked(this, |ts| ts.streaming_error(e.clone()));
            }
        }

        r
    }

    fn start(this: &mut MutexGuard<Self>, start_pos: &PropVariant) -> WinResult<()> {
        this.queue_event(
            MF::MEStreamStarted.0 as u32,
            &GUID_NULL,
            F::S_OK,
            &start_pos.to_raw(),
        )?;
        if matches!(start_pos, PropVariant::I64(_)) {
            // シーク後にEOSのままでいられると再生できない
            this.is_eos = false;
        }
        this.state = State::Started;
        Inner::dispatch_samples(this)?;

        Ok(())
    }

    fn pause(this: &mut MutexGuard<Self>) -> WinResult<()> {
        this.state = State::Paused;
        this.queue_event(
            MF::MEStreamPaused.0 as u32,
            &GUID_NULL,
            F::S_OK,
            std::ptr::null(),
        )?;

        Ok(())
    }

    fn stop(this: &mut MutexGuard<Self>) -> WinResult<()> {
        this.requests.clear();
        this.queue.clear();

        this.state = State::Stopped;
        this.queue_event(
            MF::MEStreamStopped.0 as u32,
            &GUID_NULL,
            F::S_OK,
            std::ptr::null(),
        )?;

        Ok(())
    }

    fn shutdown(this: &mut MutexGuard<Self>) -> WinResult<()> {
        this.state = State::Shutdown;

        let _ = unsafe { this.event_queue.Shutdown() };

        this.queue.clear();
        this.requests.clear();

        Ok(())
    }

    #[inline]
    fn queue_event(
        &self,
        met: u32,
        guidextendedtype: *const C::GUID,
        hrstatus: C::HRESULT,
        pvvalue: *const windows::Win32::System::Com::StructuredStorage::PROPVARIANT,
    ) -> WinResult<()> {
        unsafe {
            self.event_queue
                .QueueEventParamVar(met, guidextendedtype, hrstatus, pvvalue)
        }
    }
}

#[allow(non_snake_case)]
impl MF::IMFMediaEventGenerator_Impl for Outer {
    fn GetEvent(
        &self,
        dwflags: MF::MEDIA_EVENT_GENERATOR_GET_EVENT_FLAGS,
    ) -> WinResult<MF::IMFMediaEvent> {
        log::trace!("ElementaryStream::GetEvent");

        let queue = {
            let inner = self.inner.lock();
            inner.check_shutdown()?;
            inner.event_queue.clone()
        };

        unsafe { queue.GetEvent(dwflags.0) }
    }

    fn BeginGetEvent(
        &self,
        pcallback: Option<&MF::IMFAsyncCallback>,
        punkstate: Option<&C::IUnknown>,
    ) -> WinResult<()> {
        log::trace!("ElementaryStream::BeginGetEvent");

        let inner = self.inner.lock();
        inner.check_shutdown()?;

        unsafe { inner.event_queue.BeginGetEvent(pcallback, punkstate) }
    }

    fn EndGetEvent(&self, presult: Option<&MF::IMFAsyncResult>) -> WinResult<MF::IMFMediaEvent> {
        log::trace!("ElementaryStream::EndGetEvent");

        let inner = self.inner.lock();
        inner.check_shutdown()?;

        unsafe { inner.event_queue.EndGetEvent(presult) }
    }

    fn QueueEvent(
        &self,
        met: u32,
        guidextendedtype: *const C::GUID,
        hrstatus: C::HRESULT,
        pvvalue: *const windows::Win32::System::Com::StructuredStorage::PROPVARIANT,
    ) -> WinResult<()> {
        log::trace!("ElementaryStream::QueueEvent");

        let inner = self.inner.lock();
        inner.check_shutdown()?;

        inner.queue_event(met, guidextendedtype, hrstatus, pvvalue)
    }
}

#[allow(non_snake_case)]
impl MF::IMFMediaStream_Impl for Outer {
    fn GetMediaSource(&self) -> WinResult<MF::IMFMediaSource> {
        log::trace!("ElementaryStream::GetMediaSource");

        let inner = self.inner.lock();
        inner.check_shutdown()?;
        let source = inner.source.upgrade().ok_or(MF::MF_E_INVALIDREQUEST)?;
        Ok(source)
    }

    fn GetStreamDescriptor(&self) -> WinResult<MF::IMFStreamDescriptor> {
        let inner = self.inner.lock();
        log::trace!("ElementaryStream::GetStreamDescriptor ({:p})", &*inner);

        inner.check_shutdown()?;
        Ok(inner.stream_descriptor.clone())
    }

    fn RequestSample(&self, ptoken: Option<&C::IUnknown>) -> WinResult<()> {
        let mut inner = self.inner.lock();
        log::trace!("ElementaryStream::RequestSample {:p}", &*inner);

        let r: WinResult<()> = 'r: {
            tri!('r, inner.check_shutdown());
            if inner.state == State::Stopped {
                break 'r Err(MF::MF_E_INVALIDREQUEST.into());
            }
            if inner.is_eos && inner.queue.is_empty() {
                break 'r Err(MF::MF_E_END_OF_STREAM.into());
            }

            inner.requests.push_back(ptoken.cloned());
            tri!('r, Inner::dispatch_samples(&mut inner));

            Ok(())
        };
        if let Err(ref e) = r {
            if inner.state != State::Shutdown {
                log::debug!("error[RequestSample]: {}", e);
                Inner::source_unlocked(&mut inner, |ts| ts.streaming_error(e.clone()));
            }
        }

        Ok(())
    }
}
