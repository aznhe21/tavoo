use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use isdb::psi::table::ServiceId;

use crate::sys::player as imp;

/// TSの処理中に発生する、メインスレッドで処理するためのイベント。
#[derive(Debug, Clone)]
pub struct PlayerEvent(pub(crate) imp::PlayerEvent);

/// TSを再生するためのプレイヤー。
pub struct Player<E> {
    inner: imp::Player<E>,
}

impl<E: From<PlayerEvent>> Player<E> {
    /// ウィンドウに描画する映像プレイヤーを生成する。
    pub fn new(
        window: &winit::window::Window,
        event_loop: winit::event_loop::EventLoopProxy<E>,
    ) -> Result<Player<E>> {
        Ok(Player {
            inner: imp::Player::new(window, event_loop)?,
        })
    }
}

impl<E> Player<E> {
    /// 指定されたファイルを開き、再生を開始する。
    #[inline]
    pub fn open<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        self.inner.open(path)
    }

    /// TSの処理中にイベントループに送られた[`PlayerEvent`]を処理する。
    #[inline]
    pub fn handle_event(&self, event: PlayerEvent) -> Result<()> {
        self.inner.handle_event(event.0)
    }

    /// 再生する。
    #[inline]
    pub fn play(&self) -> Result<()> {
        self.inner.play()
    }

    /// 一時停止する。
    #[inline]
    pub fn pause(&self) -> Result<()> {
        self.inner.pause()
    }

    /// 再生、または一時停止する。
    #[inline]
    pub fn play_or_pause(&self) -> Result<()> {
        self.inner.play_or_pause()
    }

    /// 映像を再描画する。
    ///
    /// 一時停止中などで映像が描画されない場合があるため、
    /// 必要に応じてこのメソッドを呼び出す必要がある。
    #[inline]
    pub fn repaint(&self) -> Result<()> {
        self.inner.repaint()
    }

    /// 映像の大きさを設定する。
    #[inline]
    pub fn resize_video(&self, width: u32, height: u32) -> Result<()> {
        self.inner.resize_video(width, height)
    }

    /// 再生位置を取得する。
    #[inline]
    pub fn position(&self) -> Result<Duration> {
        self.inner.position()
    }

    /// 再生位置を設定する。
    #[inline]
    pub fn set_position(&self, pos: Duration) -> Result<()> {
        self.inner.set_position(pos)
    }

    /// 音量を取得する。
    #[inline]
    pub fn volume(&self) -> Result<f32> {
        self.inner.volume()
    }

    /// 音量を設定する。
    #[inline]
    pub fn set_volume(&self, value: f32) -> Result<()> {
        self.inner.set_volume(value)
    }

    /// 再生速度を取得する。
    #[inline]
    pub fn rate(&self) -> Result<f32> {
        self.inner.rate()
    }

    /// 再生速度を設定する。
    #[inline]
    pub fn set_rate(&self, value: f32) -> Result<()> {
        self.inner.set_rate(value)
    }

    /// `Player`を閉じる。
    ///
    /// 終了処理は`Drop`ではなくこちらで行うため、終了時にはこのメソッドを必ず呼び出す必要がある。
    #[inline]
    pub fn shutdown(&self) -> Result<()> {
        self.inner.shutdown()
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
    pub fn services(&self) -> isdb::filters::sorter::ServiceMap {
        self.inner.services()
    }

    /// 指定されたサービスを選択する。
    ///
    /// `service_id`に`None`を指定した場合、既定のサービスを選択する。
    #[inline]
    pub fn select_service(&self, service_id: Option<ServiceId>) -> Result<()> {
        self.inner.select_service(service_id)
    }

    /// 指定されたコンポーネントタグの映像ストリームを選択する。
    #[inline]
    pub fn select_video_stream(&self, component_tag: u8) -> Result<()> {
        self.inner.select_video_stream(component_tag)
    }

    /// 指定されたコンポーネントタグの音声ストリームを選択する。
    #[inline]
    pub fn select_audio_stream(&self, component_tag: u8) -> Result<()> {
        self.inner.select_audio_stream(component_tag)
    }
}
