//! WebViewとやり取りするためのメッセージ。
// 小数はJavaScriptが64ビットだしこちらでも64ビットにしておく

pub mod bin;
pub mod caption;
pub mod service;
pub mod str;
pub mod time;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlaybackState {
    /// 展開中。
    OpenPending,
    /// 再生中。
    Playing,
    /// 一時停止中。
    Paused,
    /// 停止中。
    Stopped,
    /// 閉じた。
    Closed,
}

/// WebViewへの通知。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "notification", rename_all = "kebab-case")]
pub enum Notification {
    /// ファイルが開かれた、または閉じられた。
    Source {
        /// 開かれたファイルへのパスだが、ファイルが閉じられた場合は`None`（`null`）。
        path: Option<String>,
    },
    /// 再生速度の範囲。
    RateRange { slowest: f64, fastest: f64 },
    /// 再生状態が更新された。
    State { state: PlaybackState },
    /// 再生位置が更新された。
    Position { position: f64 },
    /// 再生速度が更新された。
    Rate { rate: f64 },
    /// 全サービスが更新された。
    Services { services: Vec<service::Service> },
    /// 特定のサービスが更新された。
    Service { service: service::Service },
    /// サービスのイベント情報が更新された。
    Event {
        service_id: u16,
        is_present: bool,
        event: service::Event,
    },
    /// サービスが選択し直された。
    ServiceChanged { new_service_id: u16 },
    /// 字幕。
    Caption { caption: caption::Caption },
    /// 文字スーパー。
    Superimpose { superimpose: caption::Caption },
    /// エラーが発生した。
    Error { message: String },
}

/// WebViewからの要求。
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "command", rename_all = "kebab-case")]
pub enum Command {
    /// 開発者ツールを開く。
    OpenDevTools,
    /// 映像の位置を変更。
    ///
    /// 各値は相対値として0.0～1.0で指定する。
    SetVideoBounds {
        left: f64,
        top: f64,
        right: f64,
        bottom: f64,
    },
    /// 再生。
    Play,
    /// 一時停止。
    Pause,
    /// 停止。
    Stop,
    /// 再生終了。
    Close,
    /// 再生位置の変更。
    SetPosition { position: f64 },
    /// 音量の変更。
    SetVolume { volume: f64 },
    /// 再生速度の変更。
    SetRate { rate: f64 },
    /// サービスの選択。
    SelectService {
        /// `null`や`0`の場合は既定のサービスが選択される。
        service_id: Option<u16>,
    },
    /// 映像ストリームの選択。
    SelectVideoStream { component_tag: u8 },
    /// 音声ストリームの選択。
    SelectAudioStream { component_tag: u8 },
}
