mod extract;
mod player;
mod sys;

use std::time::Duration;

use winit::event::{Event, WindowEvent};
use winit::window::WindowBuilder;

#[derive(Debug, Clone)]
enum UserEvent {
    PlayerEvent(player::PlayerEvent),
}

impl From<player::PlayerEvent> for UserEvent {
    fn from(event: player::PlayerEvent) -> Self {
        UserEvent::PlayerEvent(event)
    }
}

fn main() {
    env_logger::init();

    let event_loop = winit::event_loop::EventLoopBuilder::<UserEvent>::with_user_event().build();

    let window = WindowBuilder::new()
        .with_title("TaVoo")
        .build(&event_loop)
        .unwrap();

    let mut player = player::Player::new(&window, event_loop.create_proxy()).unwrap();

    let mut modifiers = winit::event::ModifiersState::empty();
    event_loop.run(move |event, _, control_flow| {
        control_flow.set_wait();

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::Resized(size) => {
                    let _ = player.resize_video(size.width, size.height);
                }

                WindowEvent::DroppedFile(path) => match player.open(path) {
                    Ok(()) => {
                        let size = window.inner_size();
                        let _ = player.resize_video(size.width, size.height);
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
                        if let Err(e) = player.play_or_pause() {
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

                        let Some(services) = player.services() else {
                            return;
                        };
                        if let Some(service_id) = services.get_index(n).map(|(&k, _)| k) {
                            if let Err(e) = player.select_service(Some(service_id)) {
                                log::error!("select_service: {}", e)
                            }
                        }
                    }

                    key @ (winit::event::VirtualKeyCode::J | winit::event::VirtualKeyCode::K)
                        if modifiers.is_empty() =>
                    {
                        // サービス切り替え
                        let next = key == winit::event::VirtualKeyCode::K;

                        let Some(services) = player.services() else {
                            return;
                        };
                        let service_index = player
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

                            if let Err(e) = player.select_service(Some(service_id)) {
                                log::error!("select_service: {}", e)
                            }
                        }
                    }

                    key @ (winit::event::VirtualKeyCode::J | winit::event::VirtualKeyCode::K)
                        if modifiers.shift() =>
                    {
                        // 音声切り替え
                        let next = key == winit::event::VirtualKeyCode::K;

                        let Some(service) = player.selected_service() else {
                            return;
                        };
                        let index = player
                            .active_audio_tag()
                            .and_then(|audio_tag| {
                                service
                                    .audio_streams()
                                    .iter()
                                    .position(|s| s.component_tag().unwrap_or(0xFF) == audio_tag)
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
                            log::info!("new audio tag: {:02X}", new_audio_tag);

                            if let Err(e) = player.select_audio_stream(new_audio_tag) {
                                log::error!("select_audio_stream: {}", e)
                            }
                        }
                    }

                    key @ (winit::event::VirtualKeyCode::Up
                    | winit::event::VirtualKeyCode::Down) => {
                        // 音量調整
                        let up = key == winit::event::VirtualKeyCode::Up;

                        if let Ok(volume) = player.volume() {
                            let new_volume =
                                if up { volume + 0.1 } else { volume - 0.1 }.clamp(0., 1.);
                            log::info!("new volume: {}", new_volume);

                            if let Err(e) = player.set_volume(new_volume) {
                                log::error!("set_volume: {}", e);
                            }
                        }
                    }

                    key @ (winit::event::VirtualKeyCode::Left
                    | winit::event::VirtualKeyCode::Right) => {
                        // シーク
                        let forward = key == winit::event::VirtualKeyCode::Right;
                        if let Ok(pos) = player.position() {
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

                            if let Err(e) = player.set_position(new_pos) {
                                log::error!("set_position: {}", e);
                            }
                        }
                    }

                    key @ (winit::event::VirtualKeyCode::Comma
                    | winit::event::VirtualKeyCode::Period) => {
                        // 速度調整
                        let faster = key == winit::event::VirtualKeyCode::Period;

                        if let Ok(rate) = player.rate() {
                            let new_rate =
                                if faster { rate + 0.1 } else { rate - 0.1 }.clamp(0.1, 10.);
                            log::info!("new rate: {}", new_rate);

                            if let Err(e) = player.set_rate(new_rate) {
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

            Event::UserEvent(UserEvent::PlayerEvent(event)) => {
                if let Err(e) = player.handle_event(event) {
                    log::error!("player event: {}", e);
                }
            }

            Event::RedrawRequested(_) => {
                if let Err(e) = player.repaint() {
                    log::error!("repaint: {}", e);
                }
            }

            Event::LoopDestroyed => {
                player.close().unwrap();
            }

            _ => {}
        }
    });
}
