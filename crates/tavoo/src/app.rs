use std::time::Duration;

use isdb::psi::table::ServiceId;
use tavoo_components::{player, webview};
use winit::event::{Event, WindowEvent};
use winit::window::WindowBuilder;

use crate::message::{Command, Notification, PlaybackState};

/// 値がエラーの際にラベル付きブロックを抜ける。
macro_rules! tri {
    ($label:lifetime, $v:expr) => {
        match $v {
            Ok(val) => val,
            Err(err) => break $label Err(err.into()),
        }
    };
}

enum UserEvent {
    DispatchTask(Box<dyn FnOnce(&mut App) + Send>),
    WebViewCreated(anyhow::Result<()>),
}

#[derive(Debug, Clone)]
struct EventLoopProxy(winit::event_loop::EventLoopProxy<UserEvent>);

impl EventLoopProxy {
    #[inline]
    pub fn new(event_loop: &winit::event_loop::EventLoop<UserEvent>) -> EventLoopProxy {
        EventLoopProxy(event_loop.create_proxy())
    }

    #[inline]
    pub fn send_event(&self, event: UserEvent) {
        let _ = self.0.send_event(event);
    }

    /// メインスレッドで`App`を使った処理を実行する。
    #[inline]
    pub fn dispatch_task<F>(&self, f: F)
    where
        F: FnOnce(&mut App) + Send + 'static,
    {
        self.send_event(UserEvent::DispatchTask(Box::new(f)));
    }
}

#[derive(Debug, Clone)]
struct PlayerEventHandler(EventLoopProxy);

impl tavoo_components::player::EventHandler for PlayerEventHandler {
    fn on_player_event(&self, event: player::PlayerEvent) {
        self.0.dispatch_task(move |app| {
            if let Err(e) = app.player.handle_event(event) {
                log::error!("player.handle_event: {}", e);
            }
        });
    }

    fn on_ready(&self) {
        self.0.dispatch_task(|app| {
            if let Ok(range) = app.player.rate_range() {
                app.send_notification(Notification::RateRange {
                    slowest: *range.start() as f64,
                    fastest: *range.end() as f64,
                });
            }
        });
    }

    fn on_started(&self) {
        self.0.dispatch_task(|app| {
            app.send_notification(Notification::State {
                state: PlaybackState::Playing,
            });
            if let Ok(pos) = app.player.position() {
                app.send_notification(Notification::Position {
                    position: pos.as_secs_f64(),
                });
            }
        });
    }

    fn on_paused(&self) {
        self.0.dispatch_task(|app| {
            app.send_notification(Notification::State {
                state: PlaybackState::Paused,
            })
        });
    }

    fn on_stopped(&self) {
        self.0.dispatch_task(|app| {
            if app.closing {
                app.closing = false;

                app.send_notification(Notification::Source { path: None });
                app.send_notification(Notification::State {
                    state: PlaybackState::Closed,
                });
            } else {
                app.send_notification(Notification::State {
                    state: PlaybackState::Stopped,
                });
            }
        });
    }

    fn on_seek_completed(&self, position: Duration) {
        let noti = Notification::Position {
            position: position.as_secs_f64(),
        };
        self.0.dispatch_task(move |app| app.send_notification(noti));
    }

    fn on_rate_changed(&self, rate: f32) {
        let noti = Notification::Rate { rate: rate as f64 };
        self.0.dispatch_task(move |app| app.send_notification(noti));
    }

    fn on_services_updated(&self, services: &isdb::filters::sorter::ServiceMap) {
        let noti = Notification::Services {
            services: services.values().map(Into::into).collect(),
        };
        self.0.dispatch_task(move |app| app.send_notification(noti));
    }

    fn on_streams_updated(&self, service: &isdb::filters::sorter::Service) {
        let noti = Notification::Service {
            service: service.into(),
        };
        self.0.dispatch_task(move |app| app.send_notification(noti));
    }

    fn on_event_updated(&self, service: &isdb::filters::sorter::Service, is_present: bool) {
        let event = if is_present {
            service.present_event()
        } else {
            service.following_event()
        }
        .expect("is_presentで示されるイベントは必須");
        let noti = Notification::Event {
            service_id: service.service_id().get(),
            is_present,
            event: event.into(),
        };

        self.0.dispatch_task(move |app| app.send_notification(noti));
    }

    fn on_service_changed(&self, service: &isdb::filters::sorter::Service) {
        let noti = Notification::ServiceChanged {
            new_service_id: service.service_id().get(),
        };
        self.0.dispatch_task(move |app| app.send_notification(noti));
    }

    fn on_stream_changed(&self, _: tavoo_components::extract::StreamChanged) {}

    fn on_caption(&self, caption: &isdb::filters::sorter::Caption) {
        let noti = Notification::Caption {
            caption: caption.into(),
        };
        self.0.dispatch_task(move |app| app.send_notification(noti));
    }

    fn on_superimpose(&self, caption: &isdb::filters::sorter::Caption) {
        let noti = Notification::Superimpose {
            superimpose: caption.into(),
        };
        self.0.dispatch_task(move |app| app.send_notification(noti));
    }

    fn on_end_of_stream(&self) {
        self.on_stopped();
    }

    fn on_stream_error(&self, error: anyhow::Error) {
        let noti = Notification::Error {
            message: error.to_string(),
        };
        self.0.dispatch_task(move |app| app.send_notification(noti));
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Rect {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
}

impl Rect {
    pub fn new(left: f64, top: f64, right: f64, bottom: f64) -> Rect {
        Rect {
            left: left.clamp(0., 1.),
            top: top.clamp(0., 1.),
            right: right.clamp(0., 1.),
            bottom: bottom.clamp(0., 1.),
        }
    }
}

pub struct App {
    window: winit::window::Window,
    player: player::Player<PlayerEventHandler>,
    webview: webview::WebView,

    player_bounds: Rect,
    closing: bool,
}

impl App {
    #[inline]
    fn new(
        window: winit::window::Window,
        player: player::Player<PlayerEventHandler>,
        webview: webview::WebView,
    ) -> App {
        App {
            window,
            player,
            webview,

            player_bounds: Rect {
                left: 0.,
                top: 0.,
                right: 1.,
                bottom: 1.,
            },
            closing: false,
        }
    }

    /// WebViewから指示された相対位置を使用し、実際にプレイヤー部分の位置を設定する。
    ///
    /// リサイズ時等で既にウィンドウの大きさが分かっている場合は引数`size`に指定する。
    fn resize_video(&mut self, size: Option<winit::dpi::PhysicalSize<u32>>) {
        let size = size.unwrap_or_else(|| self.window.inner_size());
        // player_boundsの各値は生成時に0.0～1.0の範囲内に制限されておりasで問題ない
        let r = self.player.set_bounds(
            (self.player_bounds.left * size.width as f64) as u32,
            (self.player_bounds.top * size.height as f64) as u32,
            (self.player_bounds.right * size.width as f64) as u32,
            (self.player_bounds.bottom * size.height as f64) as u32,
        );

        if let Err(e) = r {
            log::error!("player.set_bounds: {}", e);
        }
    }

    fn send_notification(&mut self, noti: Notification) {
        let json = serde_json::to_string(&noti).expect("JSON化は常に成功すべき");
        if let Err(e) = self.webview.post_web_message(&*json) {
            log::error!("webview.post_web_message: {}", e);
        }
    }

    fn process_command(&mut self, command: Command) {
        let r = 'r: {
            match command {
                Command::OpenDevTools => {
                    tri!('r, self
                        .webview
                        .open_dev_tools()
                        .map_err(|e| format!("開発者ツールを開けません：{}", e)));
                }
                Command::SetVideoBounds {
                    left,
                    top,
                    right,
                    bottom,
                } => {
                    self.player_bounds = Rect::new(left, top, right, bottom);
                    self.resize_video(None);
                }
                Command::Play => {
                    // FIXME: 停止後に再生できない
                    tri!('r, self
                        .player
                        .play()
                        .map_err(|e| format!("再生できません：{}", e)));
                }
                Command::Pause => {
                    tri!('r, self
                        .player
                        .pause()
                        .map_err(|e| format!("一時停止できません：{}", e)));

                    if let Ok(pos) = self.player.position() {
                        self.send_notification(Notification::Position {
                            position: pos.as_secs_f64(),
                        });
                    }
                }
                Command::Stop => {
                    tri!('r, self
                        .player
                        .stop()
                        .map_err(|e| format!("停止できません：{}", e)));
                }
                Command::Close => {
                    self.closing = true;
                    tri!('r, self
                        .player
                        .close()
                        .map_err(|e| format!("ファイルを閉じることができません：{}", e)));
                }
                Command::SetPosition { position } => {
                    tri!('r, self
                        .player
                        .set_position(Duration::from_secs_f64(position))
                        .map_err(|e| format!("再生位置を設定できません：{}", e)));
                }
                Command::SetVolume { volume } => {
                    tri!('r, self
                        .player
                        .set_volume(volume as f32)
                        .map_err(|e| format!("音量を設定できません：{}", e)));
                }
                Command::SetMuted { muted } => {
                    tri!('r, self
                        .player
                        .set_muted(muted)
                        .map_err(|e| format!("ミュート状態を設定できません：{}", e)));
                }
                Command::SetRate { rate } => {
                    tri!('r, self
                        .player
                        .set_rate(rate as f32)
                        .map_err(|e| format!("再生速度を設定できません：{}", e)));
                }
                Command::SelectService { service_id } => {
                    let service_id = service_id.and_then(ServiceId::new);
                    tri!('r, self.player
                        .select_service(service_id)
                        .map_err(|e| format!("サービスを選択できません：{}", e)));
                }
                Command::SelectVideoStream { component_tag } => {
                    tri!('r, self
                        .player
                        .select_video_stream(component_tag)
                        .map_err(|e| format!("映像ストリームを選択できません：{}", e)));
                }
                Command::SelectAudioStream { component_tag } => {
                    tri!('r, self
                        .player
                        .select_audio_stream(component_tag)
                        .map_err(|e| format!("音声ストリームを選択できません：{}", e)));
                }
            }

            Ok(())
        };
        if let Err(message) = r {
            self.send_notification(Notification::Error { message });
        }
    }
}

pub fn run() -> anyhow::Result<()> {
    env_logger::init();

    let event_loop = winit::event_loop::EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = EventLoopProxy::new(&event_loop);

    let window = WindowBuilder::new()
        .with_title("TaVoo")
        .build(&event_loop)?;

    let player = player::Player::new(&window, PlayerEventHandler(proxy.clone()))?;

    let mut builder = webview::WebView::builder()
        .add_scheme("tavoo", crate::scheme::TavooHandler)
        .navigation_starting_handler(|uri| uri == "tavoo://player/content/player.html")
        .document_title_changed_handler({
            let proxy = proxy.clone();
            move |title| {
                let title = title.to_string();
                proxy.dispatch_task(move |app| {
                    app.window.set_title(&*title);
                });
            }
        })
        .web_message_received_handler({
            let proxy = proxy.clone();
            move |json| match serde_json::from_str(json) {
                Ok(command) => proxy.dispatch_task(move |app| app.process_command(command)),
                Err(e) => {
                    log::error!("WebViewからの不正なJSON：{}", e)
                }
            }
        });
    if cfg!(target_os = "windows") {
        builder = builder.arguments("--disable-features=msSmartScreenProtection");
    }

    let mut webview = builder.build(&window, {
        let proxy = proxy.clone();
        move |r| proxy.send_event(UserEvent::WebViewCreated(r))
    });
    webview.focus()?;
    webview.navigate("tavoo://player/content/player.html")?;

    let mut app = App::new(window, player, webview);

    event_loop.run(move |event, _, control_flow| {
        control_flow.set_wait();

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::Moved(_) => {
                    if let Err(e) = app.webview.notify_parent_window_moved() {
                        log::error!("webview.notify_parent_window_moved: {}", e);
                    }
                }

                WindowEvent::Resized(size) => {
                    app.resize_video(Some(size));
                    if let Err(e) = app.webview.resize(size.width, size.height) {
                        log::error!("webview.resize: {}", e);
                    }
                }

                WindowEvent::Focused(true) => {
                    if let Err(e) = app.webview.focus() {
                        log::error!("webview.focus: {}", e);
                    }
                }

                WindowEvent::DroppedFile(path) => match app.player.open(&*path) {
                    Ok(()) => {
                        app.send_notification(Notification::Source {
                            path: Some(path.to_string_lossy().into_owned()),
                        });
                        app.send_notification(Notification::State {
                            state: PlaybackState::OpenPending,
                        });
                    }
                    Err(e) => {
                        log::error!("player.open: {}", e);
                    }
                },

                WindowEvent::CloseRequested => {
                    control_flow.set_exit();
                }

                _ => {}
            },

            Event::UserEvent(event) => match event {
                UserEvent::DispatchTask(f) => f(&mut app),
                UserEvent::WebViewCreated(r) => {
                    if let Err(e) = r {
                        log::error!("WebViewの生成に失敗：{}", e);
                        control_flow.set_exit_with_code(1);
                    }
                }
            },

            Event::RedrawRequested(_) => {
                if let Err(e) = app.player.repaint() {
                    log::error!("player.repaint: {}", e);
                }
            }

            Event::LoopDestroyed => {
                if let Err(e) = app.player.close() {
                    log::error!("player.close: {}", e);
                }
                if let Err(e) = app.webview.close() {
                    log::error!("webview.close: {}", e);
                }
            }

            _ => {}
        }
    })
}
