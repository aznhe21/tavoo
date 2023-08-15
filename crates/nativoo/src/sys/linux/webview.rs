use std::path::Path;

use anyhow::Result;

use crate::webview::Handler;

pub struct ResponseBody(());

impl ResponseBody {
    #[inline]
    pub fn new(_read: Box<dyn std::io::Read>) -> ResponseBody {
        ResponseBody(())
    }

    #[inline]
    pub fn empty() -> ResponseBody {
        ResponseBody(())
    }
}

#[derive(Default)]
struct Handlers {
    file_drop_handler: Option<Box<dyn Fn(&Path)>>,
    navigation_starting_handler: Option<Box<dyn Fn(&str) -> bool>>,
    navigation_completed_handler: Option<Box<dyn Fn()>>,
    document_title_changed_handler: Option<Box<dyn Fn(&str)>>,
    web_message_received_handler: Option<Box<dyn Fn(&str)>>,
}

#[derive(Default)]
pub struct Builder {
    handlers: Handlers,
}

impl Builder {
    #[inline]
    pub fn new() -> Builder {
        Builder::default()
    }

    #[inline]
    pub fn arguments(&mut self, _args: &str) {}

    pub fn add_scheme<T>(&mut self, _name: &str, _handler: T)
    where
        T: Handler,
    {
    }

    pub fn file_drop_handler<F>(&mut self, handler: F)
    where
        F: Fn(&Path) + 'static,
    {
        self.handlers.file_drop_handler = Some(Box::new(handler));
    }

    pub fn navigation_starting_handler<F>(&mut self, handler: F)
    where
        F: Fn(&str) -> bool + 'static,
    {
        self.handlers.navigation_starting_handler = Some(Box::new(handler));
    }

    pub fn navigation_completed_handler<F>(&mut self, handler: F)
    where
        F: Fn() + 'static,
    {
        self.handlers.navigation_completed_handler = Some(Box::new(handler));
    }

    pub fn document_title_changed_handler<F>(&mut self, handler: F)
    where
        F: Fn(&str) + 'static,
    {
        self.handlers.document_title_changed_handler = Some(Box::new(handler));
    }

    pub fn web_message_received_handler<F>(&mut self, handler: F)
    where
        F: Fn(&str) + 'static,
    {
        self.handlers.web_message_received_handler = Some(Box::new(handler));
    }

    pub fn build(
        self,
        _window: &tao::window::Window,
        create_completed: Box<dyn FnOnce(Result<()>)>,
    ) -> WebView {
        let Self { mut handlers } = self;

        (create_completed)(Ok(()));
        if let Some(handler) = &mut handlers.navigation_completed_handler {
            handler();
        }

        WebView(())
    }
}

#[derive(Debug, Clone)]
pub struct WebView(());

impl WebView {
    pub fn open_dev_tools(&mut self) -> Result<()> {
        Ok(())
    }

    pub fn focus(&mut self) -> Result<()> {
        Ok(())
    }

    pub fn notify_parent_window_moved(&mut self) -> Result<()> {
        Ok(())
    }

    pub fn resize(&mut self, _width: u32, _height: u32) -> Result<()> {
        Ok(())
    }

    pub fn navigate(&mut self, _url: &str) -> Result<()> {
        Ok(())
    }

    pub fn post_web_message(&mut self, _json: &str) -> Result<()> {
        Ok(())
    }

    pub fn close(&mut self) -> Result<()> {
        Ok(())
    }
}
