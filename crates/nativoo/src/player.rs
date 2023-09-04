//! TSファイルを再生する。

use std::ops::RangeInclusive;
use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use isdb::psi::table::ServiceId;

pub use crate::extract::StreamChanged;
use crate::sys::player as imp;

/// TSの処理中に発生する、メインスレッドで処理するためのイベント。
#[derive(Debug, Clone)]
pub struct PlayerEvent(pub(crate) imp::PlayerEvent);

/// [`Player`]で発生したイベントを処理する。
///
/// 各メソッドは原則メインスレッド以外で呼ばれる。
pub trait EventHandler: Send + 'static {
    /// メインスレッドで処理するためのイベントが発生した際に呼ばれる。
    fn on_player_event(&self, event: PlayerEvent);

    /// 再生準備が整った際に呼ばれる。
    fn on_ready(&self);

    /// 再生が開始した際に呼ばれる。
    fn on_started(&self);

    /// 再生が一時停止した際に呼ばれる。
    fn on_paused(&self);

    /// 再生が停止した際に呼ばれる。
    fn on_stopped(&self);

    /// プレイヤーのシークが完了した際に呼ばれる。
    ///
    /// 引数`position`はシーク先、つまり直後に再生が開始される位置、
    /// 引数`pending`はまだ処理すべきシーク要求が残っているかどうかである。
    fn on_seek_completed(&self, position: Duration, pending: bool);

    /// 音量が変更された際に呼ばれる。
    ///
    /// 引数`volume`は新しい音量、`muted`は新しいミュート状態である。
    fn on_volume_changed(&self, volume: f32, muted: bool);

    /// 再生速度の変更が完了した際に呼ばれる。
    ///
    /// 引数`rate`は新しい再生速度である。
    fn on_rate_changed(&self, rate: f32);

    /// 映像の解像度が変更された際に呼ばれる。
    ///
    /// 引数`width`は新しい映像の幅、`height`は新しい映像の高さである。
    fn on_video_size_changed(&self, width: u32, height: u32);

    /// 音声の形式が変更された際に呼ばれる。
    fn on_audio_format_changed(&self);

    /// サービスの変更や解像度の変更によるストリームの切り替えが開始した。
    ///
    /// `on_switching_ended`が呼ばれるまで再生が進まない可能性がある。
    fn on_switching_started(&self);

    /// `on_switching_started`で開始したストリームの切り替えが終了した。
    fn on_switching_ended(&self);

    /// TSのサービス一覧が更新された際に呼ばれる。
    ///
    /// サービスの選択状態によってはこの直後にサービスが変更される可能性がある。
    fn on_services_updated(&self, services: &isdb::filters::sorter::ServiceMap);

    /// サービスのストリームが更新された際に呼ばれる。
    fn on_streams_updated(&self, service: &isdb::filters::sorter::Service);

    /// サービスのイベントが更新された際に呼ばれる。
    fn on_event_updated(&self, service: &isdb::filters::sorter::Service, is_present: bool);

    /// サービスが選択し直された際に呼ばれる。
    fn on_service_changed(&self, service: &isdb::filters::sorter::Service);

    /// 選択中サービスのストリームについて何かが変更された際に呼ばれる。
    fn on_stream_changed(&self, changed: StreamChanged);

    /// 選択中サービスで字幕パケットを受信した際に呼ばれる。
    fn on_caption(&self, pos: Option<Duration>, caption: &isdb::filters::sorter::Caption);

    /// 選択中サービスで文字スーパーのパケットを受信した際に呼ばれる。
    fn on_superimpose(&self, pos: Option<Duration>, caption: &isdb::filters::sorter::Caption);

    /// TS内の日付時刻が更新された際に呼ばれる。
    ///
    /// `timestamp`は更新された日付時刻で、1900年1月1日からの経過時間によって表される。
    fn on_timestamp_updated(&mut self, timestamp: Duration);

    /// TSを終端まで読み終えた際に呼ばれる。
    fn on_end_of_stream(&self);

    /// TS読み取り中にエラーが発生した際に呼ばれる。
    ///
    /// TSの読み取りは終了する。
    fn on_stream_error(&self, error: anyhow::Error);
}

/// デュアルモノラルでの再生方法。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

/// TSを再生するためのプレイヤー。
pub struct Player<H> {
    inner: imp::Player<H>,
}

impl<H: EventHandler + Clone> Player<H> {
    /// ウィンドウに描画する映像プレイヤーを生成する。
    pub fn new(window: &winit::window::Window, handler: H) -> Result<Player<H>> {
        Ok(Player {
            inner: imp::Player::new(window, handler)?,
        })
    }

    /// ファイルが開かれていれば`true`を返す。
    #[inline]
    pub fn is_opened(&self) -> bool {
        self.inner.is_opened()
    }

    /// 指定されたファイルを開き、再生を開始する。
    #[inline]
    pub fn open<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        self.inner.open(path)
    }

    /// TSの処理中にイベントループに送られた[`PlayerEvent`]を処理する。
    #[inline]
    pub fn handle_event(&mut self, event: PlayerEvent) -> Result<()> {
        self.inner.handle_event(event.0)
    }

    /// 再生する。
    #[inline]
    pub fn play(&mut self) -> Result<()> {
        self.inner.play()
    }

    /// 一時停止する。
    #[inline]
    pub fn pause(&mut self) -> Result<()> {
        self.inner.pause()
    }

    /// 停止する。
    #[inline]
    pub fn stop(&mut self) -> Result<()> {
        self.inner.stop()
    }

    /// 映像を再描画する。
    ///
    /// 一時停止中などで映像が描画されない場合があるため、
    /// 必要に応じてこのメソッドを呼び出す必要がある。
    #[inline]
    pub fn repaint(&mut self) -> Result<()> {
        self.inner.repaint()
    }

    /// 映像の領域を設定する。
    #[inline]
    pub fn set_bounds(&mut self, left: u32, top: u32, right: u32, bottom: u32) -> Result<()> {
        self.inner.set_bounds(left, top, right, bottom)
    }

    /// 動画の長さを取得する。
    ///
    /// 再生していない状態やリアルタイム視聴などで長さが不明な場合は`None`を返す。
    #[inline]
    pub fn duration(&self) -> Option<Duration> {
        self.inner.duration()
    }

    /// TOTとPCRによって計算される、1900年1月1日からの経過時間を返す。
    #[inline]
    pub fn timestamp(&self) -> Option<Duration> {
        self.inner.timestamp()
    }

    /// 再生位置を取得する。
    #[inline]
    pub fn position(&mut self) -> Result<Duration> {
        self.inner.position()
    }

    /// 再生位置を設定する。
    #[inline]
    pub fn set_position(&mut self, pos: Duration) -> Result<()> {
        self.inner.set_position(pos)
    }

    /// 音量を取得する。
    #[inline]
    pub fn volume(&self) -> Result<f32> {
        self.inner.volume()
    }

    /// 音量を設定する。
    #[inline]
    pub fn set_volume(&mut self, value: f32) -> Result<()> {
        self.inner.set_volume(value)
    }

    /// ミュート状態を取得する。
    #[inline]
    pub fn muted(&self) -> Result<bool> {
        self.inner.muted()
    }

    /// ミュート状態を設定する。
    #[inline]
    pub fn set_muted(&mut self, muted: bool) -> Result<()> {
        self.inner.set_muted(muted)
    }

    /// 再生速度の範囲を取得する。
    #[inline]
    pub fn rate_range(&self) -> Result<RangeInclusive<f32>> {
        self.inner.rate_range()
    }

    /// 再生速度を取得する。
    #[inline]
    pub fn rate(&self) -> Result<f32> {
        self.inner.rate()
    }

    /// 再生速度を設定する。
    #[inline]
    pub fn set_rate(&mut self, value: f32) -> Result<()> {
        self.inner.set_rate(value)
    }

    /// 映像の解像度を返す。
    #[inline]
    pub fn video_size(&self) -> Result<(u32, u32)> {
        self.inner.video_size()
    }

    /// 音声のチャンネル数を返す。
    #[inline]
    pub fn audio_channels(&self) -> Result<u8> {
        self.inner.audio_channels()
    }

    /// 現在のデュアルモノラルの再生方法を返す。
    #[inline]
    pub fn dual_mono_mode(&self) -> Result<Option<DualMonoMode>> {
        self.inner.dual_mono_mode()
    }

    /// デュアルモノラルの再生方法を設定する。
    #[inline]
    pub fn set_dual_mono_mode(&mut self, mode: DualMonoMode) -> Result<()> {
        self.inner.set_dual_mono_mode(mode)
    }

    /// `Player`を閉じる。
    ///
    /// 終了処理は`Drop`ではなくこちらで行うため、終了時にはこのメソッドを必ず呼び出す必要がある。
    #[inline]
    pub fn close(&mut self) -> Result<()> {
        self.inner.close()
    }

    /// 選択されたサービスを返す。
    ///
    /// TSを開いていない状態では`None`を返す。
    #[inline]
    pub fn selected_service(&self) -> Option<isdb::filters::sorter::Service> {
        self.inner.selected_service()
    }

    /// アクティブな映像のコンポーネントタグを返す。
    ///
    /// TSを開いていない状態、または映像ストリームにコンポーネントタグがない場合には`None`を返す。
    #[inline]
    pub fn active_video_tag(&self) -> Option<u8> {
        self.inner.active_video_tag()
    }

    /// アクティブな音声のコンポーネントタグを返す。
    ///
    /// TSを開いていない状態、または音声ストリームにコンポーネントタグがない場合には`None`を返す。
    #[inline]
    pub fn active_audio_tag(&self) -> Option<u8> {
        self.inner.active_audio_tag()
    }

    /// 現在のストリームにおける全サービスの情報を返す。
    ///
    /// TSを開いていない状態では空の連想配列を返す。
    #[inline]
    pub fn services(&self) -> Option<isdb::filters::sorter::ServiceMap> {
        self.inner.services()
    }

    /// 指定されたサービスを選択する。
    ///
    /// `service_id`に`None`を指定した場合、既定のサービスを選択する。
    #[inline]
    pub fn select_service(&mut self, service_id: Option<ServiceId>) -> Result<()> {
        self.inner.select_service(service_id)
    }

    /// 指定されたコンポーネントタグの映像ストリームを選択する。
    #[inline]
    pub fn select_video_stream(&mut self, component_tag: u8) -> Result<()> {
        self.inner.select_video_stream(component_tag)
    }

    /// 指定されたコンポーネントタグの音声ストリームを選択する。
    #[inline]
    pub fn select_audio_stream(&mut self, component_tag: u8) -> Result<()> {
        self.inner.select_audio_stream(component_tag)
    }
}

const _: () = {
    const fn assert_send<T: Send>() {}
    assert_send::<PlayerEvent>();
};
