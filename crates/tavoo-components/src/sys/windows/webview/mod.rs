mod callback;
mod options;
mod patch;
mod stream;

use std::fmt::Write;
use std::sync::Arc;

use anyhow::{Context, Result};
use fxhash::FxHashMap;
use parking_lot::Mutex;
use webview2_com_sys::Microsoft::Web::WebView2::Win32 as WV2;
use windows::core::{self as C, ComInterface, Result as WinResult};
use windows::w;
use windows::Win32::Foundation as F;
use windows::Win32::System::Com;
use windows::Win32::UI::WindowsAndMessaging as WM;
use winit::platform::windows::WindowExtWindows;

use crate::sys::com;
use crate::sys::wide_string::WideString;
use crate::sys::wrap;
use crate::webview::{Handler, Request};

use super::wide_string::WideStr;

// WebView2 1.0.1185.39で使えるものに合わせる
type ICoreWebView2 = WV2::ICoreWebView2_11;
type ICoreWebView2Controller = WV2::ICoreWebView2Controller4;
type ICoreWebView2Environment = WV2::ICoreWebView2Environment9;

/// 値を一回だけ使うためのオブジェクト。
struct Once<T>(Arc<Mutex<Option<T>>>);

impl<T> Once<T> {
    #[inline]
    pub fn new(value: T) -> Once<T> {
        Once(Arc::new(Mutex::new(Some(value))))
    }

    #[inline]
    pub fn take(&self) -> Option<T> {
        self.0.lock().take()
    }
}

impl<T> Clone for Once<T> {
    #[inline]
    fn clone(&self) -> Once<T> {
        Once(self.0.clone())
    }
}

pub struct RequestBody(Option<Com::IStream>);

impl std::io::Read for RequestBody {
    /// `buf`が`u32`を超える容量の場合、読み取られる容量は`u32::MAX`に制限される。
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if let Some(stream) = self.0.as_ref() {
            let len = buf.len().try_into().unwrap_or(u32::MAX);
            let mut read = 0;
            match unsafe { stream.Read(buf.as_mut_ptr().cast(), len, Some(&mut read)) } {
                // 32ビット未満はサポートしないので`as`で良い
                F::S_OK | F::S_FALSE => Ok(read as usize),
                hr => Err(crate::sys::error::hr_to_io(hr)),
            }
        } else {
            Ok(0)
        }
    }
}

pub struct ResponseBody(stream::ReadStream);

impl ResponseBody {
    #[inline]
    pub fn new(read: Box<dyn std::io::Read>) -> ResponseBody {
        ResponseBody(stream::ReadStream::from_boxed(read))
    }

    #[inline]
    pub fn empty() -> ResponseBody {
        static EMPTY: &[u8] = &[];
        ResponseBody(stream::ReadStream::from_read(EMPTY))
    }
}

type CreateCompleted = Once<Box<dyn FnOnce(Result<()>) -> ()>>;

#[derive(Default)]
struct Handlers {
    navigation_starting_handler: Option<Box<dyn FnMut(&str) -> bool>>,
    navigation_completed_handler: Option<Box<dyn FnMut()>>,
    document_title_changed_handler: Option<Box<dyn FnMut(&str)>>,
    web_message_received_handler: Option<Box<dyn FnMut(&str)>>,
}

#[derive(Default)]
pub struct Builder {
    env_opts: options::CoreWebView2EnvironmentOptions,
    scheme_handlers: FxHashMap<String, Box<dyn Handler>>,
    handlers: Handlers,
}

impl Builder {
    #[inline]
    pub fn new() -> Builder {
        Builder::default()
    }

    #[inline]
    pub fn arguments(&mut self, args: &str) {
        self.env_opts.additional_browser_arguments = args.into();
    }

    pub fn add_scheme<T>(&mut self, name: &str, handler: T)
    where
        T: Handler,
    {
        self.env_opts.custom_scheme_registrations.push(
            options::CoreWebView2CustomSchemeRegistration {
                scheme_name: name.into(),
                has_authority_component: true,
                ..Default::default()
            }
            .into(),
        );
        self.scheme_handlers
            .insert(name.to_string(), Box::new(handler));
    }

    pub fn navigation_starting_handler<F>(&mut self, handler: F)
    where
        F: FnMut(&str) -> bool + 'static,
    {
        self.handlers.navigation_starting_handler = Some(Box::new(handler));
    }

    pub fn navigation_completed_handler<F>(&mut self, handler: F)
    where
        F: FnMut() + 'static,
    {
        self.handlers.navigation_completed_handler = Some(Box::new(handler));
    }

    pub fn document_title_changed_handler<F>(&mut self, handler: F)
    where
        F: FnMut(&str) + 'static,
    {
        self.handlers.document_title_changed_handler = Some(Box::new(handler));
    }

    pub fn web_message_received_handler<F>(&mut self, handler: F)
    where
        F: FnMut(&str) + 'static,
    {
        self.handlers.web_message_received_handler = Some(Box::new(handler));
    }

    pub fn build(
        self,
        window: &winit::window::Window,
        create_completed: Box<dyn FnOnce(Result<()>)>,
    ) -> WebView {
        let Self {
            env_opts,
            scheme_handlers,
            handlers,
        } = self;

        let hwnd = F::HWND(window.hwnd());
        let create_completed = Once::new(create_completed);

        let options: WV2::ICoreWebView2EnvironmentOptions = env_opts.into();

        let state = Arc::new(Mutex::new(State::Pending(PendingOps::default())));
        let r = unsafe {
            WV2::CreateCoreWebView2EnvironmentWithOptions(
                None,
                None,
                &options,
                &env_completed_handler(
                    state.clone(),
                    hwnd,
                    create_completed.clone(),
                    scheme_handlers,
                    handlers,
                ),
            )
        };
        match r {
            Err(e) => {
                *state.lock() = State::Failed;

                const E_FILE_NOT_FOUND: C::HRESULT = F::ERROR_FILE_NOT_FOUND.to_hresult();
                const E_FILE_EXISTS: C::HRESULT = F::ERROR_FILE_EXISTS.to_hresult();

                let code = e.code();
                let e = anyhow::Error::new(e).context(match code {
                    E_FILE_NOT_FOUND => "Edge WebView2ランタイムが見つからない",
                    E_FILE_EXISTS => {
                        "同じ名前のファイルが存在するためユーザー用データフォルダーを生成できない"
                    }
                    F::E_ACCESSDENIED => {
                        "アクセスが拒否されたためユーザ用データフォルダーを生成できない"
                    }
                    F::E_FAIL => "Edgeランタイムを開始できない",
                    _ => "WebView2のenvironmentを生成できない",
                });

                let create_completed = create_completed
                    .take()
                    .expect("create_completedは一度しか呼ばれない");
                (create_completed)(Err(e));
            }
            Ok(()) => {}
        }

        return WebView { state };

        fn env_completed_handler(
            state: Arc<Mutex<State>>,
            hwnd: F::HWND,
            create_completed: CreateCompleted,
            scheme_handlers: FxHashMap<String, Box<dyn Handler>>,
            handlers: Handlers,
        ) -> WV2::ICoreWebView2CreateCoreWebView2EnvironmentCompletedHandler {
            callback::environment_completed_handler(move |env| {
                let r = 'r: {
                    let env = tri!('r, env);
                    unsafe {
                        tri!('r, env.CreateCoreWebView2Controller(
                            hwnd,
                            &controller_completed_handler(
                                state.clone(),
                                hwnd,
                                create_completed.clone(),
                                scheme_handlers,
                                handlers,
                                env.clone(),
                            ),
                        ));
                    }
                    Ok(())
                };

                if let Err(e) = r {
                    *state.lock() = State::Failed;

                    let create_completed = create_completed
                        .take()
                        .expect("create_completedは一度しか呼ばれない");
                    (create_completed)(Err(e));
                }

                Ok(())
            })
        }

        fn controller_completed_handler(
            state: Arc<Mutex<State>>,
            hwnd: F::HWND,
            create_completed: CreateCompleted,
            scheme_handlers: FxHashMap<String, Box<dyn Handler>>,
            handlers: Handlers,
            env: WV2::ICoreWebView2Environment,
        ) -> WV2::ICoreWebView2CreateCoreWebView2ControllerCompletedHandler {
            callback::controller_completed_handler(move |controller| {
                let r = 'r: {
                    let controller = tri!('r, controller);

                    const CONTEXT_WV2_OLD: &str = "WebView2のバージョンが古い";
                    let controller = tri!('r, controller
                            .cast::<ICoreWebView2Controller>()
                            .context(CONTEXT_WV2_OLD));
                    let env = tri!('r, env
                            .cast::<ICoreWebView2Environment>()
                            .context(CONTEXT_WV2_OLD));
                    let webview = tri!('r,
                        unsafe { tri!('r, controller.CoreWebView2()) }
                            .cast::<ICoreWebView2>()
                            .context(CONTEXT_WV2_OLD)
                    );

                    let hwnd_webview =
                        unsafe { WM::FindWindowExW(hwnd, None, w!("Chrome_WidgetWin_0"), None) };
                    if hwnd_webview == F::HWND(0) {
                        break 'r Err(C::Error::from_win32())
                            .context("WebViewの子ウィンドウが見つからない");
                    }

                    // 背景を透過させる（1/2）
                    unsafe {
                        tri!(
                            'r,
                            controller
                                .SetDefaultBackgroundColor(WV2::COREWEBVIEW2_COLOR::default())
                        );
                    }

                    // D&Dを無効化
                    // TODO: クライアント領域でD&Dを受けられなくなる
                    unsafe {
                        tri!('r, controller.SetAllowExternalDrop(false));
                    }

                    for scheme in scheme_handlers.keys() {
                        // "scheme://"で始まるURLに遷移・要求できるようにする
                        let uri = format!("{}://*", scheme);
                        let uri = WideString::from_str(&*uri);
                        unsafe {
                            tri!('r, webview.AddWebResourceRequestedFilter(
                                uri.as_pcwstr(),
                                WV2::COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
                            ));
                        }
                    }

                    let mut token =
                        windows::Win32::System::WinRT::EventRegistrationToken::default();
                    unsafe {
                        tri!('r, webview.add_NavigationCompleted(
                            &WebView::nav_handler(hwnd_webview, &controller, handlers.navigation_completed_handler),
                            &mut token,
                        ));
                        tri!('r, webview.add_WebResourceRequested(
                            &WebView::req_handler(&env, scheme_handlers),
                            &mut token,
                        ));
                    }

                    if let Some(handler) = handlers.navigation_starting_handler {
                        unsafe {
                            tri!('r, webview.add_NavigationStarting(&WebView::navigation_starting(handler), &mut token));
                        }
                    }
                    if let Some(handler) = handlers.document_title_changed_handler {
                        unsafe {
                            tri!('r, webview.add_DocumentTitleChanged(
                                &WebView::title_changed_handler(handler),
                                &mut token,
                            ));
                        }
                    }
                    if let Some(handler) = handlers.web_message_received_handler {
                        unsafe {
                            tri!('r, tri!('r, webview.Settings()).SetIsWebMessageEnabled(F::TRUE));
                            tri!('r, webview.add_WebMessageReceived(
                                &WebView::web_message_handler(handler),
                                &mut token,
                            ));
                        }
                    }

                    tri!('r, state.lock().shift(webview, controller));

                    Ok(())
                };

                let create_completed = create_completed
                    .take()
                    .expect("create_completedは一度しか呼ばれない");
                (create_completed)(r);

                Ok(())
            })
        }
    }
}

#[derive(Debug, Default)]
struct PendingOps {
    open_dev_tools: bool,
    focus: bool,
    notify_parent_window_moved: bool,
    resize: Option<(u32, u32)>,
    navigate: Option<WideString>,
    web_messages: Vec<WideString>,
}

#[derive(Debug)]
struct Inner {
    webview: ICoreWebView2,
    controller: ICoreWebView2Controller,
}

#[derive(Debug)]
enum State {
    Pending(PendingOps),
    Failed,
    Ready(Inner),
}

#[derive(Debug, Clone)]
pub struct WebView {
    state: Arc<Mutex<State>>,
}

impl State {
    /// 状態を`State::Pending`から`State::Ready`に移行する。
    ///
    /// # パニック
    ///
    /// 状態が`State::Pending`でない場合、このメソッドはパニックする。
    fn shift(&mut self, webview: ICoreWebView2, controller: ICoreWebView2Controller) -> Result<()> {
        let inner = Inner {
            webview,
            controller,
        };

        let State::Pending(ops) = std::mem::replace(self, State::Ready(inner)) else {
            unreachable!("初期化は一度だけ")
        };
        let inner = match &mut *self {
            State::Ready(v) => v,
            // Safety: 代入直後
            _ => unsafe { std::hint::unreachable_unchecked() },
        };

        if ops.open_dev_tools {
            inner.open_dev_tools()?;
        }
        if ops.focus {
            inner.focus()?;
        }
        if ops.notify_parent_window_moved {
            inner.notify_parent_window_moved()?;
        }
        if let Some((width, height)) = ops.resize {
            inner.resize(width, height)?;
        }
        if let Some(url) = ops.navigate.as_deref() {
            inner.navigate(url)?;
        }

        Ok(())
    }
}

impl Inner {
    #[inline]
    fn open_dev_tools(&self) -> WinResult<()> {
        unsafe { self.webview.OpenDevToolsWindow() }
    }

    #[inline]
    fn focus(&self) -> WinResult<()> {
        unsafe {
            self.controller
                .MoveFocus(WV2::COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC)
        }
    }

    #[inline]
    fn notify_parent_window_moved(&self) -> WinResult<()> {
        unsafe { self.controller.NotifyParentWindowPositionChanged() }
    }

    #[inline]
    fn resize(&self, width: u32, height: u32) -> WinResult<()> {
        unsafe {
            self.controller.SetBounds(F::RECT {
                left: 0,
                top: 0,
                right: width as i32,
                bottom: height as i32,
            })
        }
    }

    #[inline]
    fn navigate(&self, url: &WideStr) -> WinResult<()> {
        unsafe { self.webview.Navigate(url.as_pcwstr()) }
    }

    #[inline]
    fn post_web_message(&self, message: &WideStr) -> WinResult<()> {
        unsafe { self.webview.PostWebMessageAsJson(message.as_pcwstr()) }
    }

    #[inline]
    fn close(&self) -> WinResult<()> {
        unsafe { self.controller.Close() }
    }
}

impl WebView {
    fn nav_handler(
        hwnd_webview: F::HWND,
        controller: &ICoreWebView2Controller,
        mut handler: Option<Box<dyn FnMut()>>,
    ) -> WV2::ICoreWebView2NavigationCompletedEventHandler {
        let mut loaded = false;
        let controller = controller.clone();

        callback::navigation_completed_event_handler(move |_sender, _args| {
            if !loaded {
                unsafe {
                    // 背景を透過させる（2/2）
                    let ex_style = WM::GetWindowLongPtrW(hwnd_webview, WM::GWL_EXSTYLE)
                        | WM::WS_EX_TRANSPARENT.0 as isize;
                    WM::SetWindowLongPtrW(hwnd_webview, WM::GWL_EXSTYLE, ex_style);
                }

                unsafe { controller.SetIsVisible(true)? };

                loaded = true;
            }

            if let Some(handler) = &mut handler {
                handler();
            }

            Ok(())
        })
    }

    fn req_handler(
        env: &ICoreWebView2Environment,
        mut scheme_handlers: FxHashMap<String, Box<dyn Handler>>,
    ) -> WV2::ICoreWebView2WebResourceRequestedEventHandler {
        let env = env.clone();

        callback::web_resource_requested_event_handler(move |_, args| {
            let args = args.ok_or(F::E_POINTER)?;
            let req = unsafe { args.Request()? };

            let res = 'res: {
                let uri = wrap::wrap(|s| unsafe { req.Uri(s) })?.to_string()?;
                let uri: http::Uri = match uri.parse() {
                    Err(e) => {
                        log::warn!("不正なURL（'{}'）：{}", uri, e);
                        break 'res None;
                    }
                    Ok(uri) => uri,
                };
                log::trace!("独自スキームへのアクセス：{}", uri);

                let method = wrap::wrap(|s| unsafe { req.Method(s) })?.to_string()?;
                if !method.eq_ignore_ascii_case("GET") {
                    log::warn!("独自スキームへのGET以外のアクセス：{}", method);
                    break 'res None;
                }

                let Some(handler) = uri
                    .scheme_str()
                    .and_then(|scheme| scheme_handlers.get_mut(scheme))
                else {
                    log::debug!("不明なスキーム：{}", uri.scheme_str().unwrap_or(""));
                    break 'res None;
                };

                let res = handler.handle(build_request(&req, uri, &*method)?);
                Some(res)
            }
            .unwrap_or_else(|| {
                crate::webview::Response::builder()
                    .status(http::StatusCode::BAD_REQUEST)
                    .body(crate::webview::ResponseBody::empty())
                    .unwrap()
            });

            unsafe { args.SetResponse(&build_response(&env, res)?)? };
            Ok(())
        })
    }

    fn navigation_starting(
        mut handler: Box<dyn FnMut(&str) -> bool>,
    ) -> WV2::ICoreWebView2NavigationStartingEventHandler {
        callback::navigation_starting_event_handler(move |_, args| {
            let args = args.ok_or(F::E_POINTER)?;

            let uri = wrap::wrap(|a| unsafe { args.Uri(a) })?.to_string()?;
            if !handler(&*uri) {
                log::trace!("'{}'への遷移を取り消し", uri);
                unsafe { args.SetCancel(F::TRUE)? };
            }
            Ok(())
        })
    }

    fn title_changed_handler(
        mut handler: Box<dyn FnMut(&str)>,
    ) -> WV2::ICoreWebView2DocumentTitleChangedEventHandler {
        callback::document_title_changed_event_handler(move |webview, _| {
            let webview = webview.ok_or(F::E_POINTER)?;

            let title = wrap::wrap(|a| unsafe { webview.DocumentTitle(a) })?.to_string()?;
            handler(&*title);
            Ok(())
        })
    }

    fn web_message_handler(
        mut handler: Box<dyn FnMut(&str)>,
    ) -> WV2::ICoreWebView2WebMessageReceivedEventHandler {
        callback::web_message_received_event_handler(move |_, args| {
            let args = args.ok_or(F::E_POINTER)?;

            let json = wrap::wrap(|a| unsafe { args.WebMessageAsJson(a) })?.to_string()?;
            handler(&*json);
            Ok(())
        })
    }

    const FAILED_MSG: &str = "WebViewは使用不可能";

    pub fn open_dev_tools(&mut self) -> Result<()> {
        match &mut *self.state.lock() {
            State::Pending(ops) => ops.open_dev_tools = true,
            State::Failed => return Err(anyhow::Error::msg(Self::FAILED_MSG)),
            State::Ready(inner) => inner.open_dev_tools()?,
        }
        Ok(())
    }

    pub fn focus(&mut self) -> Result<()> {
        match &mut *self.state.lock() {
            State::Pending(ops) => ops.focus = true,
            State::Failed => return Err(anyhow::Error::msg(Self::FAILED_MSG)),
            State::Ready(inner) => inner.focus()?,
        }
        Ok(())
    }

    pub fn notify_parent_window_moved(&mut self) -> Result<()> {
        match &mut *self.state.lock() {
            State::Pending(ops) => ops.notify_parent_window_moved = true,
            State::Failed => return Err(anyhow::Error::msg(Self::FAILED_MSG)),
            State::Ready(inner) => inner.notify_parent_window_moved()?,
        }
        Ok(())
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        match &mut *self.state.lock() {
            State::Pending(ops) => ops.resize = Some((width, height)),
            State::Failed => return Err(anyhow::Error::msg(Self::FAILED_MSG)),
            State::Ready(inner) => inner.resize(width, height)?,
        }
        Ok(())
    }

    pub fn navigate(&mut self, url: &str) -> Result<()> {
        let url = url.into();

        match &mut *self.state.lock() {
            State::Pending(ops) => ops.navigate = Some(url),
            State::Failed => return Err(anyhow::Error::msg(Self::FAILED_MSG)),
            State::Ready(inner) => inner.navigate(&*url)?,
        }
        Ok(())
    }

    pub fn post_web_message(&mut self, json: &str) -> Result<()> {
        let json = json.into();

        match &mut *self.state.lock() {
            State::Pending(ops) => ops.web_messages.push(json),
            State::Failed => return Err(anyhow::Error::msg(Self::FAILED_MSG)),
            State::Ready(inner) => inner.post_web_message(&*json)?,
        }
        Ok(())
    }

    pub fn close(&mut self) -> Result<()> {
        match &mut *self.state.lock() {
            // 生成中のWebViewにも生成失敗後にもやれることはない
            State::Pending(_) | State::Failed => {}
            State::Ready(inner) => inner.close()?,
        }
        Ok(())
    }
}

fn build_request(
    req: &WV2::ICoreWebView2WebResourceRequest,
    uri: http::Uri,
    method: &str,
) -> WinResult<Request> {
    let mut builder = http::Request::builder().uri(uri);

    builder = builder.method(method);

    let headers = unsafe { req.Headers()? };
    let iter = unsafe { headers.GetIterator()? };
    if wrap::wrap(|v| unsafe { iter.HasCurrentHeader(v) })? {
        loop {
            let (name, value) = wrap::wrap2(|a, b| unsafe { iter.GetCurrentHeader(a, b) })?;
            builder = builder.header(&*name.to_string()?, &*value.to_string()?);

            if !wrap::wrap(|v| unsafe { iter.MoveNext(v) })? {
                break;
            }
        }
    }

    let req = builder.body(()).map_err(|e| {
        log::debug!("不正なリクエスト：{}", e);
        F::E_FAIL
    })?;
    Ok(req)
}

fn build_response(
    env: &ICoreWebView2Environment,
    res: crate::webview::Response,
) -> WinResult<WV2::ICoreWebView2WebResourceResponse> {
    let mut headers = String::with_capacity(256);
    for (k, v) in res.headers() {
        if let Ok(v) = v.to_str() {
            let _ = writeln!(headers, "{}: {}", k, v);
        }
    }

    let (parts, body) = res.into_parts();
    let content: Com::IStream = body.0 .0.into();

    let reason = WideString::from_str(parts.status.as_str());
    let headers = WideString::from_str(&*headers);
    let res = unsafe {
        env.CreateWebResourceResponse(
            &content,
            parts.status.as_u16() as i32,
            reason.as_pcwstr(),
            headers.as_pcwstr(),
        )?
    };
    Ok(res)
}
