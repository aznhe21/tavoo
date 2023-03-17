use std::collections::VecDeque;

use parking_lot::{Mutex, MutexGuard};
use windows::core::{self as C, implement, AsImpl, Interface};
use windows::Win32::Foundation as F;
use windows::Win32::Media::KernelStreaming::GUID_NULL;
use windows::Win32::Media::MediaFoundation as MF;

use super::source::TransportStream;
use super::utils::{PropVariant, WinResult};

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
        unsafe {
            let event_queue = MF::MFCreateEventQueue()?;
            let inner = Mutex::new(Inner {
                source: source.intf().downgrade().unwrap(),
                event_queue,

                stream_descriptor,

                state: State::Stopped,
                is_active: false,
                is_eos: false,

                samples: VecDeque::new(),
                requests: VecDeque::new(),
            });
            Ok(ElementaryStream(Outer { inner }.into()))
        }
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
    pub fn deliver_payload(&self, sample: MF::IMFSample) -> WinResult<()> {
        Inner::deliver_payload(&mut self.inner(), sample)
    }

    #[inline]
    pub fn is_active(&self) -> bool {
        self.inner().is_active
    }

    #[inline]
    pub fn activate(&self, active: bool) {
        Inner::activate(&mut self.inner(), active)
    }

    #[inline]
    pub fn start(&self, start_pos: Option<&PropVariant>) -> WinResult<()> {
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

#[implement(MF::IMFMediaStream)]
struct Outer {
    inner: Mutex<Inner>,
}

struct Inner {
    source: C::Weak<MF::IMFMediaSource>,
    event_queue: MF::IMFMediaEventQueue,

    stream_descriptor: MF::IMFStreamDescriptor,

    state: State,
    is_active: bool,
    is_eos: bool,

    samples: VecDeque<MF::IMFSample>,
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
        self.is_active && !self.is_eos && self.samples.len() < SAMPLE_QUEUE
    }

    fn end_of_stream(this: &mut MutexGuard<Self>) -> WinResult<()> {
        this.is_eos = true;
        Inner::dispatch_samples(this)?;

        Ok(())
    }

    fn deliver_payload(this: &mut MutexGuard<Self>, sample: MF::IMFSample) -> WinResult<()> {
        this.samples.push_back(sample);
        Inner::dispatch_samples(this)?;

        Ok(())
    }

    fn dispatch_samples(this: &mut MutexGuard<Self>) -> WinResult<()> {
        fn dispatch_samples(this: &mut MutexGuard<Inner>) -> WinResult<()> {
            unsafe {
                if this.state != State::Started {
                    return Ok(());
                }

                while !this.samples.is_empty() && !this.requests.is_empty() {
                    let sample = this.samples.pop_front().unwrap();
                    let token = this.requests.pop_front().unwrap();

                    if let Some(token) = token {
                        sample.SetUnknown(&MF::MFSampleExtension_Token, &token)?;
                    }

                    this.event_queue.QueueEventParamUnk(
                        MF::MEMediaSample.0 as u32,
                        &GUID_NULL,
                        F::S_OK,
                        &sample,
                    )?;
                }

                if this.samples.is_empty() && this.is_eos {
                    log::debug!("sample exhausted ({:p})", this);
                    this.event_queue.QueueEventParamVar(
                        MF::MEEndOfStream.0 as u32,
                        &GUID_NULL,
                        F::S_OK,
                        std::ptr::null(),
                    )?;
                    Inner::source_unlocked(this, |ts| ts.enqueue_end_of_stream())?;
                } else if this.needs_data() {
                    Inner::source_unlocked(this, |ts| ts.request_sample());
                }

                Ok(())
            }
        }

        let r = dispatch_samples(this);
        if let Err(ref e) = r {
            if this.state != State::Shutdown {
                log::debug!("error[dispatch_samples]: {}", e);
                Inner::source_unlocked(this, |ts| ts.streaming_error(e.clone()));
            }
        }

        r
    }

    fn activate(this: &mut MutexGuard<Self>, active: bool) {
        if active == this.is_active {
            return;
        }

        this.is_active = active;

        if !active {
            this.samples.clear();
            this.requests.clear();
        }
    }

    fn start(this: &mut MutexGuard<Self>, start_pos: Option<&PropVariant>) -> WinResult<()> {
        this.queue_event(
            MF::MEStreamStarted.0 as u32,
            &GUID_NULL,
            F::S_OK,
            match start_pos {
                Some(start_pos) => &start_pos.to_raw(),
                None => std::ptr::null(),
            },
        )?;
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
        this.samples.clear();

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
        unsafe {
            this.state = State::Shutdown;

            let _ = this.event_queue.Shutdown();

            this.samples.clear();
            this.requests.clear();

            Ok(())
        }
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
        unsafe {
            log::trace!("ElementaryStream::GetEvent");

            let queue = {
                let inner = self.inner.lock();
                inner.check_shutdown()?;
                inner.event_queue.clone()
            };

            queue.GetEvent(dwflags.0)
        }
    }

    fn BeginGetEvent(
        &self,
        pcallback: &Option<MF::IMFAsyncCallback>,
        punkstate: &Option<C::IUnknown>,
    ) -> WinResult<()> {
        unsafe {
            log::trace!("ElementaryStream::BeginGetEvent");

            let inner = self.inner.lock();
            inner.check_shutdown()?;

            inner
                .event_queue
                .BeginGetEvent(pcallback.as_ref(), punkstate.as_ref())
        }
    }

    fn EndGetEvent(&self, presult: &Option<MF::IMFAsyncResult>) -> WinResult<MF::IMFMediaEvent> {
        unsafe {
            log::trace!("ElementaryStream::EndGetEvent");

            let inner = self.inner.lock();
            inner.check_shutdown()?;

            inner.event_queue.EndGetEvent(presult.as_ref())
        }
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
        log::trace!("ElementaryStream::GetStreamDescriptor ({:p})", self);

        let inner = self.inner.lock();
        inner.check_shutdown()?;
        Ok(inner.stream_descriptor.clone())
    }

    fn RequestSample(&self, ptoken: &Option<C::IUnknown>) -> WinResult<()> {
        fn request_sample(
            inner: &mut MutexGuard<Inner>,
            ptoken: &Option<C::IUnknown>,
        ) -> WinResult<()> {
            inner.check_shutdown()?;
            if inner.state == State::Stopped || !inner.is_active {
                return Err(MF::MF_E_INVALIDREQUEST.into());
            }
            if inner.is_eos && inner.samples.is_empty() {
                return Err(MF::MF_E_END_OF_STREAM.into());
            }

            inner.requests.push_back(ptoken.clone());
            Inner::dispatch_samples(inner)?;

            Ok(())
        }

        log::trace!("ElementaryStream::RequestSample {:p}", self);

        let mut inner = self.inner.lock();

        let r = request_sample(&mut inner, ptoken);
        if let Err(ref e) = r {
            if inner.state != State::Shutdown {
                log::debug!("error[RequestSample]: {}", e);
                Inner::source_unlocked(&mut inner, |ts| ts.streaming_error(e.clone()));
            }
        }

        Ok(())
    }
}
