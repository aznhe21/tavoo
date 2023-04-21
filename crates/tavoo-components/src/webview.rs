//! 動画とオーバーレイしてUIを構築するためのWebView。

use std::io;

use anyhow::Result;
pub use http;

use crate::sys::webview as imp;

/// 独自スキームを処理するための[`Handler`]から返されるレスポンスの内容。
pub struct ResponseBody(pub(crate) imp::ResponseBody);

impl ResponseBody {
    /// [`Read`][`io::Read`]を実装するオブジェクトからレスポンス内容を生成する。
    #[inline]
    pub fn new<R: io::Read + 'static>(read: R) -> ResponseBody {
        ResponseBody(imp::ResponseBody::new(Box::new(read)))
    }

    /// 空のレスポンス内容を生成する。
    #[inline]
    pub fn empty() -> ResponseBody {
        ResponseBody(imp::ResponseBody::empty())
    }
}

/// 独自スキームを処理するための[`Handler`]に渡されるリクエスト。
pub type Request<T = ()> = http::Request<T>;

/// 独自スキームを処理するための[`Handler`]から返されるレスポンス。
pub type Response<T = ResponseBody> = http::Response<T>;

/// WebViewからのリクエストを処理する。
pub trait Handler: 'static {
    /// WebViewからのリクエストを処理してレスポンスを生成する。
    fn handle(&mut self, request: Request) -> Response;
}

impl<F> Handler for F
where
    F: FnMut(Request) -> Response + 'static,
{
    #[inline]
    fn handle(&mut self, request: Request) -> Response {
        (self)(request)
    }
}

/// WebViewに設定を与える。
pub struct Builder {
    inner: imp::Builder,
}

impl Builder {
    /// WebViewに設定を与えるための`Builder`を生成する。
    #[inline]
    pub fn new() -> Builder {
        Builder {
            inner: imp::Builder::new(),
        }
    }

    /// WebViewの実装に対して引数を与える。
    pub fn arguments(mut self, args: &str) -> Builder {
        self.inner.arguments(args);
        self
    }

    /// 独自スキームとそこにアクセスがあった際のハンドラーを追加する。
    ///
    /// このスキームにはGETリクエストのみを送信することができる。
    /// それ以外のリクエストを送信した場合、400等のエラーが返される。
    pub fn add_scheme<T>(mut self, name: &str, handler: T) -> Builder
    where
        T: Handler,
    {
        self.inner.add_scheme(name, handler);
        self
    }

    /// 遷移が始まる際のハンドラーを指定する。
    ///
    /// ハンドラーから`false`が返った場合、遷移は取り消される。
    pub fn navigation_starting_handler<F>(mut self, handler: F) -> Builder
    where
        F: FnMut(&str) -> bool + 'static,
    {
        self.inner.navigation_starting_handler(handler);
        self
    }

    /// コンテンツのタイトルが変更された際のハンドラーを指定する。
    pub fn document_title_changed_handler<F>(mut self, handler: F) -> Builder
    where
        F: FnMut(&str) + 'static,
    {
        self.inner.document_title_changed_handler(handler);
        self
    }

    /// スクリプトからメッセージを受信した際のハンドラーを指定する。
    ///
    /// 引数にはJSONが渡されるため、serde_json等を使ってパースすると良い。
    pub fn web_message_received_handler<F>(mut self, handler: F) -> Builder
    where
        F: FnMut(&str) + 'static,
    {
        self.inner.web_message_received_handler(handler);
        self
    }

    /// この設定を使って[`Window`]上にWebViewを生成する。
    ///
    /// WebViewの生成は非同期であるが、生成が完了する前でも戻り値の`WebView`を使った操作が可能。
    ///
    /// 生成が完了したら`create_completed`が呼ばれる。引数の`Result`によって生成時のエラーを捉えることができる。
    ///
    /// [`Window`]: winit::window::Window
    #[inline]
    pub fn build<F>(self, window: &winit::window::Window, create_completed: F) -> WebView
    where
        F: FnOnce(Result<()>) + 'static,
    {
        let inner = self.inner.build(window, Box::new(create_completed));
        WebView { inner }
    }
}

/// TaVoo用のWebView。
///
/// 一般的なWebViewとは以下の点が異なる。
/// - 背景が透過される
/// - WebView領域へのドラッグ＆ドロップは無効化される
pub struct WebView {
    inner: imp::WebView,
}

impl WebView {
    /// WebViewに設定を与えるための`Builder`を生成する。
    #[inline]
    pub fn builder() -> Builder {
        Builder::new()
    }

    /// 指定された[`Window`]上にWebViewを生成する。
    ///
    /// WebViewの生成は非同期であるが、生成が完了する前でも戻り値の`WebView`を使った操作が可能。
    ///
    /// 生成が完了したら`create_completed`が呼ばれる。引数の`Result`によって生成時のエラーを捉えることができる。
    ///
    /// [`Window`]: winit::window::Window
    #[inline]
    pub fn new<F>(window: &winit::window::Window, create_completed: F) -> WebView
    where
        F: FnOnce(Result<()>) + 'static,
    {
        Self::builder().build(window, create_completed)
    }

    /// 開発者ツールを開く。
    #[inline]
    pub fn open_dev_tools(&mut self) -> Result<()> {
        self.inner.open_dev_tools()
    }

    /// WebViewにフォーカスを移す。
    #[inline]
    pub fn focus(&mut self) -> Result<()> {
        self.inner.focus()
    }

    /// WebViewに親ウィンドウが移動したことを通知する。
    #[inline]
    pub fn notify_parent_window_moved(&mut self) -> Result<()> {
        self.inner.notify_parent_window_moved()
    }

    /// WebViewの大きさを変える。
    #[inline]
    pub fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        self.inner.resize(width, height)
    }

    /// WebViewを`url`に遷移させる。
    #[inline]
    pub fn navigate(&mut self, url: &str) -> Result<()> {
        self.inner.navigate(url)
    }

    /// JSON形式のメッセージをWebViewに送る。
    #[inline]
    pub fn post_web_message(&mut self, json: &str) -> Result<()> {
        self.inner.post_web_message(json)
    }

    /// WebViewを閉じる。
    #[inline]
    pub fn close(&mut self) -> Result<()> {
        self.inner.close()
    }
}
