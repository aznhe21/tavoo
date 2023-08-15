use std::path::PathBuf;

use nativoo::webview::{http, Handler, Request, Response, ResponseBody};

/// `tavoo://`を扱うハンドラー。
pub struct TavooHandler;

impl Handler for TavooHandler {
    fn handle(&self, req: Request) -> Response {
        let Some(host) = req.uri().host() else {
            log::error!("ホストがない：{}", req.uri());
            return Response::builder()
                .status(http::StatusCode::BAD_REQUEST)
                .body(ResponseBody::empty())
                .unwrap();
        };

        let raw_path = req.uri().path();
        if !raw_path.starts_with("/") || host.contains("..") || raw_path.contains("..") {
            log::error!("パスが不正：{}", req.uri().path());
            return Response::builder()
                .status(http::StatusCode::BAD_REQUEST)
                .body(ResponseBody::empty())
                .unwrap();
        }
        let raw_path = &raw_path[1..];

        // TODO: リリース版ではアーカイブにまとめる
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("omni");
        path.push(host);
        path.push(raw_path);

        let (file, metadata) = match std::fs::File::open(&*path).and_then(|f| {
            let metadata = f.metadata()?;
            Ok((f, metadata))
        }) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                log::warn!("ファイルがない：{}", req.uri());
                return Response::builder()
                    .status(http::StatusCode::NOT_FOUND)
                    .body(ResponseBody::empty())
                    .unwrap();
            }
            Err(e) => {
                log::warn!("ファイルを開けない：{}", e);
                return Response::builder()
                    .status(http::StatusCode::INTERNAL_SERVER_ERROR)
                    .body(ResponseBody::empty())
                    .unwrap();
            }
            Ok(tup) => tup,
        };

        let mime = mime_guess::from_path(&path).first_or_text_plain();
        Response::builder()
            .header("Content-Type", mime.to_string())
            .header("Content-Length", metadata.len())
            .body(ResponseBody::new(file))
            .unwrap()
    }
}
