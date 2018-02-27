#[macro_use]
extern crate clap;
extern crate env_logger;
extern crate glutin;
extern crate image;
extern crate linked_hash_map;
#[macro_use]
extern crate log;
extern crate reqwest;
extern crate toml;
extern crate xdg;

pub mod args;
#[macro_use]
pub mod context;
pub mod buffer;
pub mod config;
pub mod coord;
pub mod map_view;
pub mod map_view_gl;
pub mod program;
pub mod texture;
pub mod tile;
pub mod tile_cache;
pub mod tile_atlas;
pub mod tile_loader;
pub mod tile_source;

use coord::ScreenCoord;
use glutin::{ControlFlow, ElementState, Event, GlContext, MouseButton, MouseScrollDelta, VirtualKeyCode, WindowEvent};
use map_view_gl::MapViewGl;
use std::time::{Duration, Instant};
use tile_source::TileSource;


#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Action {
    Nothing,
    Redraw,
    Close,
}

#[derive(Copy, Clone, Debug, PartialEq)]
struct InputState {
    mouse_position: (f64, f64),
    mouse_pressed: bool,
}

fn handle_event(event: &Event, map: &mut MapViewGl, input_state: &mut InputState, sources: &mut TileSources) -> Action {
    match *event {
        Event::Awakened => Action::Redraw,
        Event::WindowEvent{ref event, ..} => match *event {
            WindowEvent::Closed => Action::Close,
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => {
                input_state.mouse_pressed = true;
                Action::Nothing
            },
            WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Left, .. } => {
                input_state.mouse_pressed = false;
                Action::Nothing
            },
            WindowEvent::CursorMoved { position: (x, y), .. } => {
                if input_state.mouse_pressed {
                    map.move_pixel(
                        input_state.mouse_position.0 - x,
                        input_state.mouse_position.1 - y,
                    );
                    input_state.mouse_position = (x, y);
                    Action::Redraw
                } else {
                    input_state.mouse_position = (x, y);
                    Action::Nothing
                }
            },
            WindowEvent::MouseWheel { delta, modifiers, .. } => {
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(dx, dy) => {
                        // filter strange wheel events with huge values.
                        // (maybe this is just a personal touchpad driver issue)
                        if dx.abs() < 16.0 && dy.abs() < 16.0 {
                            //TODO find a sensible line height value (servo (the glutin port) uses 38)
                            (dx, dy * 38.0)
                        } else {
                            (0.0, 0.0)
                        }
                    },
                    MouseScrollDelta::PixelDelta(dx, dy) => (dx, dy),
                };

                //TODO add option for default mouse wheel behavior (scroll or zoom?)
                //TODO add option to reverse scroll/zoom direction

                if modifiers.ctrl {
                    map.move_pixel(f64::from(-dx), f64::from(-dy));
                } else {
                    map.zoom_at(
                        ScreenCoord::new(
                            input_state.mouse_position.0,
                            input_state.mouse_position.1,
                        ),
                        f64::from(dy) * (1.0 / 320.0),
                    );
                }
                Action::Redraw
            },
            WindowEvent::KeyboardInput {
                input: glutin::KeyboardInput {
                    state: glutin::ElementState::Pressed,
                    virtual_keycode: Some(keycode),
                    modifiers,
                    .. },
                .. } => {
                match keycode {
                    VirtualKeyCode::Escape => {
                        Action::Close
                    },
                    VirtualKeyCode::PageUp => {
                        sources.switch_to_prev();
                        Action::Redraw
                    },
                    VirtualKeyCode::PageDown => {
                        sources.switch_to_next();
                        Action::Redraw
                    },
                    VirtualKeyCode::Left => {
                        map.move_pixel(-50.0, 0.0);
                        Action::Redraw
                    },
                    VirtualKeyCode::Right => {
                        map.move_pixel(50.0, 0.0);
                        Action::Redraw
                    },
                    VirtualKeyCode::Up => {
                        map.move_pixel(0.0, -50.0);
                        Action::Redraw
                    },
                    VirtualKeyCode::Down => {
                        map.move_pixel(0.0, 50.0);
                        Action::Redraw
                    },
                    VirtualKeyCode::Add => {
                        if modifiers.ctrl {
                            map.change_tile_zoom_offset(1.0);
                        } else {
                            map.step_zoom(1, 0.5);
                        }
                        Action::Redraw
                    },
                    VirtualKeyCode::Subtract => {
                        if modifiers.ctrl {
                            map.change_tile_zoom_offset(-1.0);
                        } else {
                            map.step_zoom(-1, 0.5);
                        }
                        Action::Redraw
                    },
                    _ => Action::Nothing,
                }
            },
            WindowEvent::Refresh => {
                Action::Redraw
            },
            WindowEvent::Resized(w, h) => {
                map.set_viewport_size(w, h);
                Action::Redraw
            },
            _ => Action::Nothing,
        },
        _ => Action::Nothing,
    }
}

fn dur_to_sec(dur: Duration) -> f64 {
    dur.as_secs() as f64 + f64::from(dur.subsec_nanos()) * 1e-9
}

fn main() {
    env_logger::init();

    let matches = args::parse();

    let config = if let Some(config_path) = matches.value_of_os("config") {
            config::Config::from_toml_file(config_path).unwrap()
        } else {
            config::Config::load().unwrap()
        };

    let mut sources = TileSources::new(config.tile_sources()).unwrap();

    let mut events_loop = glutin::EventsLoop::new();
    let builder = glutin::WindowBuilder::new()
        .with_title(format!("DeltaMap - {}", sources.current_name()));

    let gl_context = glutin::ContextBuilder::new();
    let gl_window = glutin::GlWindow::new(builder, gl_context, &events_loop).unwrap();
    let window = gl_window.window();

    let _ = unsafe { gl_window.make_current() };
    let cx = context::Context::from_gl_window(&gl_window);

    let mut map = {
        let proxy = events_loop.create_proxy();

        map_view_gl::MapViewGl::new(
            &cx,
            window.get_inner_size().unwrap(),
            move || { proxy.wakeup().unwrap(); },
            !matches.is_present("offline"),
            !matches.is_present("sync"),
        )
    };

    let mut input_state = InputState {
        mouse_position: (0.0, 0.0),
        mouse_pressed: false,
    };

    let fps: f64 = matches.value_of("fps").map(|s| s.parse().unwrap()).unwrap_or_else(|| config.fps());
    let duration_per_frame = Duration::from_millis((1000.0 / fps - 0.5).max(0.0).floor() as u64);
    info!("milliseconds per frame: {}", dur_to_sec(duration_per_frame) * 1000.0);

    // estimated draw duration
    let mut est_draw_dur = duration_per_frame;
    let mut last_draw = Instant::now();
    let mut increase_atlas_size = true;

    loop {
        let start_source_id = sources.current().id();
        let mut redraw = false;
        let mut close = false;

        events_loop.run_forever(|event| {
            match handle_event(&event, &mut map, &mut input_state, &mut sources) {
                Action::Close => close = true,
                Action::Redraw => redraw = true,
                Action::Nothing => {},
            }
            ControlFlow::Break
        });

        if close {
            break;
        }

        events_loop.poll_events(|event| {
            match handle_event(&event, &mut map, &mut input_state, &mut sources) {
                Action::Close => {
                    close = true;
                    return;
                },
                Action::Redraw => {
                    redraw = true;
                },
                Action::Nothing => {},
            }
        });

        if close {
            break;
        }

        {
            let diff = last_draw.elapsed();
            if diff + est_draw_dur * 2 < duration_per_frame {
                if let Some(dur) = duration_per_frame.checked_sub(est_draw_dur * 2) {
                    std::thread::sleep(dur);

                    events_loop.poll_events(|event| {
                        match handle_event(&event, &mut map, &mut input_state, &mut sources) {
                            Action::Close => {
                                close = true;
                                return;
                            },
                            Action::Redraw => {
                                redraw = true;
                            },
                            Action::Nothing => {},
                        }
                    });

                    if close {
                        break;
                    }
                }
            }
        }

        if redraw {
            let draw_start = Instant::now();
            let draw_result = map.draw(sources.current());
            let draw_dur = draw_start.elapsed();

            let _ = gl_window.swap_buffers();

            // Move glClear call out of the critical path.
            //TODO do not call glClear when drawing fills the whole screen anyway
            cx.clear_color((0.2, 0.2, 0.2, 1.0));


            //TODO increase atlas size earlier to avoid excessive copying to the GPU
            //TODO increase max tile cache size?
            increase_atlas_size = {
                match (draw_result, increase_atlas_size) {
                    (Err(draws), true) if draws > 1 => {
                        map.increase_atlas_size().is_ok()
                    },
                    (Ok(draws), true) if draws > 1 => {
                        map.increase_atlas_size().is_ok()
                    },
                    _ => increase_atlas_size,
                }
            };

            last_draw = Instant::now();

            debug!("draw: {} sec (est {} sec)", dur_to_sec(draw_dur), dur_to_sec(est_draw_dur));

            est_draw_dur = if draw_dur > est_draw_dur {
                draw_dur
            } else {
                (draw_dur / 4) + ((est_draw_dur / 4) * 3)
            };
        }

        // set window title
        if sources.current().id() != start_source_id {
            window.set_title(&format!("DeltaMap - {}", sources.current_name()));
        }
    }
}

struct TileSources<'a> {
    current_index: usize,
    sources: &'a [(String, TileSource)],
}

impl<'a> TileSources<'a> {
    pub fn new(sources: &'a [(String, TileSource)]) -> Option<TileSources> {
        if sources.is_empty() {
            None
        } else {
            Some(TileSources {
                current_index: 0,
                sources: sources,
            })
        }
    }

    pub fn current(&self) -> &TileSource {
        &self.sources[self.current_index].1
    }

    pub fn current_name(&self) -> &str {
        &self.sources[self.current_index].0
    }

    pub fn switch_to_next(&mut self) {
        self.current_index = (self.current_index + 1) % self.sources.len();
    }

    pub fn switch_to_prev(&mut self) {
        self.current_index = (self.current_index + self.sources.len().saturating_sub(1)) % self.sources.len();
    }
}
