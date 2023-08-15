use std::borrow::Cow;
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use anyhow::{Context, Result};
use gtk::prelude::Cast;
use gtk::traits::{BoxExt, ContainerExt, WidgetExt};
use javascriptcore::ValueExt;
use tao::platform::unix::WindowExtUnix;
use webkit2gtk::{
    NavigationPolicyDecisionExt, SecurityManagerExt, SettingsExt, URIRequestExt,
    URISchemeRequestExt, URISchemeResponseExt, UserContentManagerExt, WebContextExt,
    WebInspectorExt, WebViewExt,
};

use crate::webview::Handler;

pub struct ResponseBody(std::io::Result<Cow<'static, [u8]>>);

impl ResponseBody {
    #[inline]
    pub fn new(mut read: Box<dyn std::io::Read>) -> ResponseBody {
        let mut vec = Vec::new();
        ResponseBody(read.read_to_end(&mut vec).map(|_| vec.into()))
    }

    #[inline]
    pub fn empty() -> ResponseBody {
        static EMPTY: &[u8] = &[];
        ResponseBody(Ok(EMPTY.into()))
    }
}

pub struct Builder {
    context: webkit2gtk::WebContext,
    manager: webkit2gtk::UserContentManager,
    webview: webkit2gtk::WebView,
}

impl Builder {
    pub fn new() -> Builder {
        let context = webkit2gtk::WebContext::builder().build();
        let manager = webkit2gtk::UserContentManager::new();

        manager.register_script_message_handler("tavoo");
        manager.add_script(&webkit2gtk::UserScript::new(
            WebView::INIT_SCRIPT,
            webkit2gtk::UserContentInjectedFrames::TopFrame,
            webkit2gtk::UserScriptInjectionTime::Start,
            &[],
            &[],
        ));

        let mut builder = webkit2gtk::WebView::builder();
        builder = builder.web_context(&context);
        builder = builder.user_content_manager(&manager);

        let webview = builder.build();

        webview.set_background_color(&gdk::RGBA::new(0., 0., 0., 0.));
        if let Some(settings) = webkit2gtk::WebViewExt::settings(&webview) {
            settings.set_enable_developer_extras(true);
        }

        Builder {
            context,
            manager,
            webview,
        }
    }

    #[inline]
    pub fn arguments(&mut self, _args: &str) {}

    pub fn add_scheme<T>(&mut self, name: &str, handler: T)
    where
        T: Handler,
    {
        let Some(sec) = self.context.security_manager() else { return };
        sec.register_uri_scheme_as_secure(name);

        self.context.register_uri_scheme(name, move |req| {
            let res = 'res: {
                let uri = tri!('res, req.uri().ok_or_else(|| "URIがない".to_string()));
                let uri: http::Uri = tri!('res, uri
                    .parse()
                    .map_err(|e| format!("不正なURL（'{}'）：{}", uri, e)));
                log::trace!("独自スキームへのアクセス：{}", uri);

                let mut builder = http::Request::builder().uri(uri);

                if let Some(method) = req.http_method().as_deref() {
                    if !method.eq_ignore_ascii_case("GET") {
                        break 'res Err(format!("独自スキームへのGET以外のアクセス：{}", method));
                    }

                    builder = builder.method(method);
                }

                if let Some(headers_mut) = builder.headers_mut() {
                    if let Some(headers) = req.http_headers() {
                        headers.foreach(|k, v| {
                            let Ok(k) = http::HeaderName::from_bytes(k.as_bytes()) else { return };
                            let Ok(v) = http::HeaderValue::from_bytes(v.as_bytes()) else { return };
                            headers_mut.insert(k, v);
                        });
                    }
                }

                let req = tri!('res, builder
                    .body(())
                    .map_err(|e| format!("不正なリクエスト：{}", e)));
                let res = handler.handle(req);
                Ok(res)
            };
            match res {
                Ok(res) => match &res.body().0 .0 {
                    Ok(body) => {
                        let input = gio::MemoryInputStream::from_bytes(&(**body).into());
                        let resp = webkit2gtk::URISchemeResponse::new(&input, body.len() as i64);

                        resp.set_status(res.status().as_u16() as u32, Some(res.status().as_str()));
                        if let Some(content_type) = res
                            .headers()
                            .get(http::header::CONTENT_TYPE)
                            .and_then(|v| v.to_str().ok())
                        {
                            resp.set_content_type(content_type);
                        }

                        let headers = soup::MessageHeaders::new(soup::MessageHeadersType::Response);
                        for (k, v) in res.headers() {
                            if let Ok(v) = v.to_str() {
                                headers.append(k.as_str(), v);
                            }
                        }
                        resp.set_http_headers(&headers);

                        req.finish_with_response(&resp);
                    }
                    Err(e) => {
                        req.finish_error(&mut glib::Error::new(
                            glib::FileError::Io,
                            &*e.to_string(),
                        ));
                    }
                },
                Err(e) => {
                    log::warn!("{}", e);
                    req.finish_error(&mut glib::Error::new(glib::FileError::Exist, &*e));
                }
            }
        });
    }

    pub fn file_drop_handler<F>(&mut self, handler: F)
    where
        F: Fn(&Path) + 'static,
    {
        let paths = Rc::new(Cell::new(Vec::new()));

        self.webview.connect_drag_data_received({
            let paths = paths.clone();
            move |_, _, _, _, data, _, _| {
                let incoming = data
                    .uris()
                    .into_iter()
                    .filter_map(|uri| uri.as_str().strip_prefix("file://").map(PathBuf::from))
                    .collect::<Vec<_>>();
                if !incoming.is_empty() {
                    paths.set(incoming);
                }
            }
        });
        self.webview.connect_drag_drop(move |_, _, _, _, _| {
            for path in paths.take() {
                (handler)(&*path);
            }
            true
        });
    }

    pub fn navigation_starting_handler<F>(&mut self, handler: F)
    where
        F: Fn(&str) -> bool + 'static,
    {
        self.webview
            .connect_decide_policy(move |_, decision, kind| {
                if kind != webkit2gtk::PolicyDecisionType::NavigationAction {
                    return true;
                }
                let Some(decision) =
                    decision.dynamic_cast_ref::<webkit2gtk::NavigationPolicyDecision>()
                else {
                    return true;
                };
                let Some(action) = decision.navigation_action() else { return true; };
                let Some(req) = action.request() else { return true };
                let Some(uri) = req.uri() else { return true };
                (handler)(&*uri)
            });
    }

    pub fn navigation_completed_handler<F>(&mut self, handler: F)
    where
        F: Fn() + 'static,
    {
        self.webview.connect_load_changed(move |_, e| {
            if e == webkit2gtk::LoadEvent::Finished {
                (handler)();
            }
        });
    }

    pub fn document_title_changed_handler<F>(&mut self, handler: F)
    where
        F: Fn(&str) + 'static,
    {
        self.webview.connect_title_notify(move |webview| {
            (handler)(webview.title().as_deref().unwrap_or(""));
        });
    }

    pub fn web_message_received_handler<F>(&mut self, handler: F)
    where
        F: Fn(&str) + 'static,
    {
        self.manager
            .connect_script_message_received(None, move |_, msg| {
                let Some(js) = msg.js_value() else { return };
                let Some(json) = js.to_json(0) else { return };
                (handler)(&*json);
            });
    }

    pub fn build(
        self,
        window: &tao::window::Window,
        create_completed: Box<dyn FnOnce(Result<()>)>,
    ) -> WebView {
        let gwin = window.gtk_window();
        let Self { webview, .. } = self;

        if let Some(vbox) = window.default_vbox() {
            vbox.pack_start(&webview, true, true, 0);
        } else {
            gwin.add(&webview);
        }
        webview.grab_focus();

        gwin.show_all();
        (create_completed)(Ok(()));

        WebView { webview }
    }
}

#[derive(Debug, Clone)]
pub struct WebView {
    webview: webkit2gtk::WebView,
}

impl WebView {
    const INIT_SCRIPT: &str = "\
      Object.defineProperty(\
        window,\
        \"ipc\",\
        {\
          value: new class extends EventTarget {\
            postMessage(data) {\
              window.webkit.messageHandlers.tavoo.postMessage(data);\
            }\
          }\
        }\
      );\
    ";

    pub fn open_dev_tools(&mut self) -> Result<()> {
        let inspector = self.webview.inspector().context("")?;
        inspector.show();
        Ok(())
    }

    pub fn focus(&mut self) -> Result<()> {
        self.webview.grab_focus();
        Ok(())
    }

    pub fn notify_parent_window_moved(&mut self) -> Result<()> {
        Ok(())
    }

    pub fn resize(&mut self, _: u32, _: u32) -> Result<()> {
        Ok(())
    }

    pub fn navigate(&mut self, url: &str) -> Result<()> {
        self.webview.load_uri(url);
        Ok(())
    }

    pub fn post_web_message(&mut self, json: &str) -> Result<()> {
        let script =
            format!("window.ipc.dispatchEvent(new MessageEvent(\"message\", {{ data: {json} }}));");
        self.webview
            .run_javascript(&*script, gio::Cancellable::NONE, |_| {});
        Ok(())
    }

    pub fn close(&mut self) -> Result<()> {
        Ok(())
    }
}
