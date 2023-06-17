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
    Source {
        /// 開かれたファイルへのパスだが、ファイルが閉じられた場合は`None`（`null`）。
        path: Option<String>,
    },
    /// 音量。
    Volume { volume: f64, muted: bool },
    /// 再生速度の範囲。
    RateRange { slowest: f64, fastest: f64 },
    /// パケットの各種エラー数。
    PacketCount {
        format_error: u64,
        transport_error: u64,
        continuity_error: u64,
        scrambled: u64,
    },
    /// 動画の長さ。
    Duration {
        /// 秒単位の長さ。
        ///
        /// 再生していない状態やリアルタイム視聴などで長さが不明な場合は`null`となる。
        duration: Option<f64>,
    },
    /// 再生状態が更新された。
    State { state: PlaybackState },
    /// 再生位置が更新された。
    Position { position: f64 },
    /// すべてのシークが完了した。
    SeekCompleted,
    /// 再生速度が更新された。
    Rate { rate: f64 },
    /// 映像の解像度が変更された。
    VideoSize { width: u32, height: u32 },
    /// 音声のチャンネル数が変更された。
    AudioChannels { num_channels: u8 },
    /// デュアルモノラルの再生方法が更新された。
    DualMonoMode { mode: Option<DualMonoMode> },
    /// ストリームの切り替えが開始した。
    SwitchingStarted,
    /// ストリームの切り替えが終了した。
    SwitchingEnded,
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
    ServiceChanged {
        new_service_id: u16,
        video_component_tag: Option<u8>,
        audio_component_tag: Option<u8>,
    },
    /// ストリームが変更された。
    StreamChanged {
        video_component_tag: u8,
        audio_component_tag: u8,
    },
    /// 字幕。
    Caption {
        /// 字幕を表示すべき再生位置。
        pos: Option<f64>,
        /// 字幕データ。
        caption: caption::Caption,
    },
    /// 文字スーパー。
    Superimpose {
        /// 文字スーパーを表示すべき再生位置。
        pos: Option<f64>,
        /// 文字スーパーのデータ。
        caption: caption::Caption,
    },
    /// TSの日付時刻。
    Timestamp { timestamp: time::Timestamp },
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
    /// パケットの各種エラー数をゼロに戻す。
    ResetPacketCount,
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
    /// ミュート状態の変更。
    SetMuted { muted: bool },
    /// 再生速度の変更。
    SetRate { rate: f64 },
    /// デュアルモノラルの再生方法の変更。
    SetDualMonoMode { mode: DualMonoMode },
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
