use std::collections::VecDeque;

use parking_lot::Mutex;
use windows::core::{implement, AsImpl};
use windows::Win32::Foundation as F;
use windows::Win32::Media::MediaFoundation as MF;

use super::utils::WinResult;

#[derive(Debug, Clone)]
pub struct AsyncQueue(MF::IMFAsyncCallback);

// Safety: 内包するIMFAsyncCallbackはInnerであり、InnerはSendであるため安全
unsafe impl Send for AsyncQueue {}

#[implement(MF::IMFAsyncCallback)]
struct Inner {
    queue: Mutex<VecDeque<Box<dyn FnOnce()>>>,
}

impl AsyncQueue {
    #[inline]
    pub fn new() -> AsyncQueue {
        AsyncQueue(
            Inner {
                queue: Mutex::new(VecDeque::new()),
            }
            .into(),
        )
    }

    #[inline]
    fn inner(&self) -> &Inner {
        self.0.as_impl()
    }

    pub fn process_queue(&self) -> WinResult<()> {
        unsafe {
            if !self.inner().queue.lock().is_empty() {
                MF::MFPutWorkItem(MF::MFASYNC_CALLBACK_QUEUE_STANDARD, &self.0, None)?;
            }
            Ok(())
        }
    }

    pub fn enqueue<F: FnOnce() + Send + 'static>(&self, f: F) -> WinResult<()> {
        self.inner().queue.lock().push_back(Box::new(f));

        self.process_queue()?;
        Ok(())
    }
}

#[allow(non_snake_case)]
impl MF::IMFAsyncCallback_Impl for Inner {
    fn GetParameters(&self, _: *mut u32, _: *mut u32) -> WinResult<()> {
        Err(F::E_NOTIMPL.into())
    }

    fn Invoke(&self, _: Option<&MF::IMFAsyncResult>) -> WinResult<()> {
        let f = self.queue.lock().pop_front();
        if let Some(f) = f {
            f();
        }

        Ok(())
    }
}
