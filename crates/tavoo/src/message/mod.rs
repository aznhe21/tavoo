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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DualMonoMode {
    /// チャンネル1を左右スピーカーに出力する。
    Left,
    /// チャンネル2を左右スピーカーに出力する。
    Right,
    /// チャンネル1を左スピーカーに、チャンネル2を右スピーカーに出力する。
    Stereo,
    /// チャンネル1とチャンネル2を混合して左右スピーカーに出力する。
    Mix,
}

impl From<tavoo_components::player::DualMonoMode> for DualMonoMode {
    fn from(mode: tavoo_components::player::DualMonoMode) -> DualMonoMode {
        match mode {
            tavoo_components::player::DualMonoMode::Left => DualMonoMode::Left,
            tavoo_components::player::DualMonoMode::Right => DualMonoMode::Right,
            tavoo_components::player::DualMonoMode::Stereo => DualMonoMode::Stereo,
            tavoo_components::player::DualMonoMode::Mix => DualMonoMode::Mix,
        }
    }
}

impl From<DualMonoMode> for tavoo_components::player::DualMonoMode {
    fn from(mode: DualMonoMode) -> tavoo_components::player::DualMonoMode {
        match mode {
            DualMonoMode::Left => tavoo_components::player::DualMonoMode::Left,
            DualMonoMode::Right => tavoo_components::player::DualMonoMode::Right,
            DualMonoMode::Stereo => tavoo_components::player::DualMonoMode::Stereo,
            DualMonoMode::Mix => tavoo_components::player::DualMonoMode::Mix,
        }
    }
}

/// WebViewへの通知。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "notification", rename_all = "kebab-case")]
pub enum Notification {
    /// ファイルが開かれた、または閉じられた。
    #[serde(rename_all = "camelCase")]
    Source {
        /// 開かれたファイルへのパスだが、ファイルが閉じられた場合は`None`（`null`）。
        path: Option<String>,
    },
    /// 音量。
    #[serde(rename_all = "camelCase")]
    Volume { volume: f64, muted: bool },
    /// 再生速度の範囲。
    #[serde(rename_all = "camelCase")]
    RateRange { slowest: f64, fastest: f64 },
    /// 動画の長さ。
    #[serde(rename_all = "camelCase")]
    Duration {
        /// 秒単位の長さ。
        ///
        /// 再生していない状態やリアルタイム視聴などで長さが不明な場合は`null`となる。
        duration: Option<f64>,
    },
    /// 再生状態が更新された。
    #[serde(rename_all = "camelCase")]
    State { state: PlaybackState },
    /// 再生位置が更新された。
    #[serde(rename_all = "camelCase")]
    Position { position: f64 },
    /// すべてのシークが完了した。
    #[serde(rename_all = "camelCase")]
    SeekCompleted,
    /// 再生速度が更新された。
    #[serde(rename_all = "camelCase")]
    Rate { rate: f64 },
    /// 映像の解像度が変更された。
    #[serde(rename_all = "camelCase")]
    VideoSize { width: u32, height: u32 },
    /// 音声のチャンネル数が変更された。
    #[serde(rename_all = "camelCase")]
    AudioChannels { num_channels: u8 },
    /// デュアルモノラルの再生方法が更新された。
    #[serde(rename_all = "camelCase")]
    DualMonoMode { mode: Option<DualMonoMode> },
    /// ストリームの切り替えが開始した。
    #[serde(rename_all = "camelCase")]
    SwitchingStarted,
    /// ストリームの切り替えが終了した。
    #[serde(rename_all = "camelCase")]
    SwitchingEnded,
    /// 全サービスが更新された。
    #[serde(rename_all = "camelCase")]
    Services { services: Vec<service::Service> },
    /// 特定のサービスが更新された。
    #[serde(rename_all = "camelCase")]
    Service { service: service::Service },
    /// サービスのイベント情報が更新された。
    #[serde(rename_all = "camelCase")]
    Event {
        service_id: u16,
        is_present: bool,
        event: service::Event,
    },
    /// サービスが選択し直された。
    #[serde(rename_all = "camelCase")]
    ServiceChanged {
        new_service_id: u16,
        video_component_tag: Option<u8>,
        audio_component_tag: Option<u8>,
    },
    /// ストリームが変更された。
    #[serde(rename_all = "camelCase")]
    StreamChanged {
        video_component_tag: u8,
        audio_component_tag: u8,
    },
    /// 字幕。
    #[serde(rename_all = "camelCase")]
    Caption {
        /// 字幕を表示すべき再生位置。
        pos: Option<f64>,
        /// 字幕データ。
        caption: caption::Caption,
    },
    /// 文字スーパー。
    #[serde(rename_all = "camelCase")]
    Superimpose {
        /// 文字スーパーを表示すべき再生位置。
        pos: Option<f64>,
        /// 文字スーパーのデータ。
        caption: caption::Caption,
    },
    /// TSの日付時刻。
    #[serde(rename_all = "camelCase")]
    Timestamp { timestamp: time::Timestamp },
    /// エラーが発生した。
    #[serde(rename_all = "camelCase")]
    Error { message: String },
}

/// WebViewからの要求。
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "command", rename_all = "kebab-case")]
pub enum Command {
    /// 開発者ツールを開く。
    #[serde(rename_all = "camelCase")]
    OpenDevTools,
    /// 映像の位置を変更。
    ///
    /// 各値は相対値として`0.0`～`1.0`で指定する。
    #[serde(rename_all = "camelCase")]
    SetVideoBounds {
        left: f64,
        top: f64,
        right: f64,
        bottom: f64,
    },
    /// 再生。
    #[serde(rename_all = "camelCase")]
    Play,
    /// 一時停止。
    #[serde(rename_all = "camelCase")]
    Pause,
    /// 停止。
    #[serde(rename_all = "camelCase")]
    Stop,
    /// 再生終了。
    #[serde(rename_all = "camelCase")]
    Close,
    /// 再生位置の変更。
    #[serde(rename_all = "camelCase")]
    SetPosition { position: f64 },
    /// 音量の変更。
    #[serde(rename_all = "camelCase")]
    SetVolume { volume: f64 },
    /// ミュート状態の変更。
    #[serde(rename_all = "camelCase")]
    SetMuted { muted: bool },
    /// 再生速度の変更。
    #[serde(rename_all = "camelCase")]
    SetRate { rate: f64 },
    /// デュアルモノラルの再生方法の変更。
    #[serde(rename_all = "camelCase")]
    SetDualMonoMode { mode: DualMonoMode },
    /// サービスの選択。
    #[serde(rename_all = "camelCase")]
    SelectService {
        /// `null`や`0`の場合は既定のサービスが選択される。
        service_id: Option<u16>,
    },
    /// 映像ストリームの選択。
    #[serde(rename_all = "camelCase")]
    SelectVideoStream { component_tag: u8 },
    /// 音声ストリームの選択。
    #[serde(rename_all = "camelCase")]
    SelectAudioStream { component_tag: u8 },
}
