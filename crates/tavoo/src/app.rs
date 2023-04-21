use std::time::Duration;

use tavoo_components::player;
use winit::event::{Event, WindowEvent};
use winit::window::WindowBuilder;

enum UserEvent {
    DispatchTask(Box<dyn FnOnce(&mut App) + Send>),
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
                log::error!("player event: {}", e);
            }
        });
    }

    fn on_ready(&self) {
        // TODO: UIに通知
    }

    fn on_started(&self) {
        // TODO: UIに通知
    }

    fn on_paused(&self) {
        // TODO: UIに通知
    }

    fn on_stopped(&self) {
        // TODO: UIに通知
    }

    fn on_seek_completed(&self, _position: Duration) {
        // TODO: UIに通知
    }

    fn on_rate_changed(&self, _rate: f32) {
        // TODO: UIに通知
    }

    fn on_services_updated(&self, _services: &isdb::filters::sorter::ServiceMap) {
        // TODO: UIに通知
    }

    fn on_streams_updated(&self, _service: &isdb::filters::sorter::Service) {
        // TODO: UIに通知
    }

    fn on_event_updated(&self, _service: &isdb::filters::sorter::Service, _is_present: bool) {
        // TODO: UIに通知
        // let service_id = self
        //     .handler
        //     .selected_stream()
        //     .as_ref()
        //     .map(|ss| ss.service_id);
        // if service_id != Some(service.service_id()) {
        //     return;
        // }

        // if is_present {
        //     if let Some(name) = service.present_event().and_then(|e| e.name.as_ref()) {
        //         log::info!("event changed: {}", name.display(Default::default()));
        //     }
        // }
    }

    fn on_service_changed(&self, service: &isdb::filters::sorter::Service) {
        // TODO: UIに通知
        log::info!(
            "service changed: {} ({:04X})",
            service.service_name().display(Default::default()),
            service.service_id()
        );
    }

    fn on_stream_changed(&self, _: tavoo_components::extract::StreamChanged) {}

    fn on_caption(&self, _caption: &isdb::filters::sorter::Caption) {
        // TODO: UIに通知
        // let service_id = {
        //     let selected_stream = self.handler.selected_stream();
        //     let Some(selected_stream) = selected_stream.as_ref() else {
        //         return;
        //     };
        //     selected_stream.service_id
        // };
        // let decode_opts = if self.handler.services()[&service_id].is_oneseg() {
        //     isdb::eight::decode::Options::ONESEG_CAPTION
        // } else {
        //     isdb::eight::decode::Options::CAPTION
        // };

        // for data_unit in caption.data_units() {
        //     let isdb::pes::caption::DataUnit::StatementBody(caption) = data_unit else {
        //         continue;
        //     };

        //     let caption = caption.to_string(decode_opts);
        //     if !caption.is_empty() {
        //         log::info!("caption: {}", caption);
        //     }
        // }
    }

    fn on_superimpose(&self, _caption: &isdb::filters::sorter::Caption) {
        // TODO: UIに通知
        // let service_id = {
        //     let selected_stream = self.handler.selected_stream();
        //     let Some(selected_stream) = selected_stream.as_ref() else {
        //         return;
        //     };
        //     selected_stream.service_id
        // };
        // let decode_opts = if self.handler.services()[&service_id].is_oneseg() {
        //     isdb::eight::decode::Options::ONESEG_CAPTION
        // } else {
        //     isdb::eight::decode::Options::CAPTION
        // };

        // for data_unit in caption.data_units() {
        //     let isdb::pes::caption::DataUnit::StatementBody(caption) = data_unit else {
        //         continue;
        //     };

        //     if !caption.is_empty() {
        //         log::info!("superimpose: {:?}", caption.display(decode_opts));
        //     }
        // }
    }

    fn on_end_of_stream(&self) {
        // TODO: UIに通知
    }

    fn on_stream_error(&self, error: anyhow::Error) {
        // TODO: UIに通知
        log::error!("stream error: {}", error);
    }
}

pub struct App {
    window: winit::window::Window,
    player: player::Player<PlayerEventHandler>,
}

pub fn run() -> anyhow::Result<()> {
    env_logger::init();

    let event_loop = winit::event_loop::EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = EventLoopProxy::new(&event_loop);

    let window = WindowBuilder::new()
        .with_title("TaVoo")
        .build(&event_loop)?;

    let player = player::Player::new(&window, PlayerEventHandler(proxy))?;

    let mut app = App { window, player };

    let mut modifiers = winit::event::ModifiersState::empty();
    event_loop.run(move |event, _, control_flow| {
        control_flow.set_wait();

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::Resized(size) => {
                    let _ = app.player.set_bounds(0, 0, size.width, size.height);
                }

                WindowEvent::DroppedFile(path) => match app.player.open(path) {
                    Ok(()) => {
                        let size = app.window.inner_size();
                        let _ = app.player.set_bounds(0, 0, size.width, size.height);
                    }
                    Err(e) => {
                        log::error!("{}", e);
                    }
                },

                WindowEvent::ModifiersChanged(state) => modifiers = state,
                WindowEvent::KeyboardInput {
                    input:
                        winit::event::KeyboardInput {
                            state: winit::event::ElementState::Pressed,
                            virtual_keycode: Some(keycode),
                            ..
                        },
                    ..
                } => match keycode {
                    winit::event::VirtualKeyCode::Space => {
                        if let Err(e) = app.player.play_or_pause() {
                            log::error!("{}", e);
                        }
                    }

                    key @ (winit::event::VirtualKeyCode::Key1
                    | winit::event::VirtualKeyCode::Key2
                    | winit::event::VirtualKeyCode::Key3
                    | winit::event::VirtualKeyCode::Key4
                    | winit::event::VirtualKeyCode::Key5
                    | winit::event::VirtualKeyCode::Key6
                    | winit::event::VirtualKeyCode::Key7
                    | winit::event::VirtualKeyCode::Key8
                    | winit::event::VirtualKeyCode::Key9) => {
                        // 0～8番目のサービス選択
                        let n = key as usize - winit::event::VirtualKeyCode::Key1 as usize;

                        let Some(services) = app.player.services() else {
                            return;
                        };
                        if let Some(service_id) = services.get_index(n).map(|(&k, _)| k) {
                            if let Err(e) = app.player.select_service(Some(service_id)) {
                                log::error!("select_service: {}", e)
                            }
                        }
                    }

                    key @ (winit::event::VirtualKeyCode::J | winit::event::VirtualKeyCode::K)
                        if modifiers.is_empty() =>
                    {
                        // サービス切り替え
                        let next = key == winit::event::VirtualKeyCode::K;

                        let Some(services) = app.player.services() else {
                            return;
                        };
                        let service_index = app
                            .player
                            .selected_service()
                            .and_then(|svc| services.get_index_of(&svc.service_id()))
                            .unwrap_or(0);
                        let new_service = if next {
                            service_index.checked_add(1)
                        } else {
                            service_index.checked_sub(1)
                        }
                        .and_then(|new_index| {
                            services.get_index(new_index).map(|(&k, _)| (new_index, k))
                        });
                        if let Some((new_index, service_id)) = new_service {
                            log::info!("new service: {}", new_index);

                            if let Err(e) = app.player.select_service(Some(service_id)) {
                                log::error!("select_service: {}", e)
                            }
                        }
                    }

                    key @ (winit::event::VirtualKeyCode::J | winit::event::VirtualKeyCode::K)
                        if modifiers.shift() =>
                    {
                        // 音声切り替え
                        let next = key == winit::event::VirtualKeyCode::K;

                        let Some(service) = app.player.selected_service() else {
                            return;
                        };
                        let index =
                            app.player
                                .active_audio_tag()
                                .and_then(|audio_tag| {
                                    service.audio_streams().iter().position(|s| {
                                        s.component_tag().unwrap_or(0xFF) == audio_tag
                                    })
                                })
                                .unwrap_or(0);
                        let new_audio_tag = if next {
                            index.checked_add(1)
                        } else {
                            index.checked_sub(1)
                        }
                        .and_then(|new_index| service.audio_streams().get(new_index))
                        .map(|new_audio_stream| new_audio_stream.component_tag().unwrap_or(0xFF));

                        if let Some(new_audio_tag) = new_audio_tag {
                            let ac = service.present_event().and_then(|event| {
                                event
                                    .audio_components
                                    .iter()
                                    .find(|ac| ac.component_tag == new_audio_tag)
                            });
                            if let Some(ac) = ac {
                                if !ac.text.is_empty() {
                                    log::info!(
                                        "new audio: {}",
                                        ac.text.display(Default::default())
                                    );
                                } else {
                                    log::info!("new audio: {}", ac.lang_code);
                                }
                            } else {
                                log::info!("new audio tag: {:02X}", new_audio_tag);
                            }

                            if let Err(e) = app.player.select_audio_stream(new_audio_tag) {
                                log::error!("select_audio_stream: {}", e)
                            }
                        }
                    }

                    key @ (winit::event::VirtualKeyCode::Up
                    | winit::event::VirtualKeyCode::Down) => {
                        // 音量調整
                        let up = key == winit::event::VirtualKeyCode::Up;

                        if let Ok(volume) = app.player.volume() {
                            let new_volume =
                                if up { volume + 0.1 } else { volume - 0.1 }.clamp(0., 1.);
                            log::info!("new volume: {}", new_volume);

                            if let Err(e) = app.player.set_volume(new_volume) {
                                log::error!("set_volume: {}", e);
                            }
                        }
                    }

                    key @ (winit::event::VirtualKeyCode::Left
                    | winit::event::VirtualKeyCode::Right) => {
                        // シーク
                        let forward = key == winit::event::VirtualKeyCode::Right;
                        if let Ok(pos) = app.player.position() {
                            let dur = if modifiers.shift() {
                                Duration::from_secs(60)
                            } else {
                                Duration::from_secs(1)
                            };
                            let new_pos = if forward {
                                pos + dur
                            } else {
                                pos.saturating_sub(dur)
                            };
                            log::info!("new position: {:?}", new_pos);

                            if let Err(e) = app.player.set_position(new_pos) {
                                log::error!("set_position: {}", e);
                            }
                        }
                    }

                    key @ (winit::event::VirtualKeyCode::Comma
                    | winit::event::VirtualKeyCode::Period) => {
                        // 速度調整
                        let faster = key == winit::event::VirtualKeyCode::Period;

                        if let Ok(rate) = app.player.rate() {
                            let new_rate =
                                if faster { rate + 0.1 } else { rate - 0.1 }.clamp(0.1, 10.);
                            log::info!("new rate: {}", new_rate);

                            if let Err(e) = app.player.set_rate(new_rate) {
                                log::error!("set_rate: {}", e);
                            }
                        }
                    }

                    _ => {}
                },

                WindowEvent::CloseRequested => {
                    control_flow.set_exit();
                }

                _ => {}
            },

            Event::UserEvent(event) => match event {
                UserEvent::DispatchTask(f) => f(&mut app),
            },

            Event::RedrawRequested(_) => {
                if let Err(e) = app.player.repaint() {
                    log::error!("repaint: {}", e);
                }
            }

            Event::LoopDestroyed => {
                if let Err(e) = app.player.close() {
                    log::error!("close: {}", e);
                }
            }

            _ => {}
        }
    })
}
