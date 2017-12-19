#[cfg(target_os = "android")]
#[macro_use]
extern crate android_glue;

#[macro_use]
extern crate serde_derive;

extern crate glutin;
extern crate image;
extern crate linked_hash_map;
extern crate reqwest;
extern crate serde;
extern crate toml;


#[macro_use]
mod context;

mod buffer;
mod config;
mod coord;
mod map_view;
mod map_view_gl;
mod program;
mod texture;
mod tile;
mod tile_cache;
mod tile_cache_gl;
mod tile_loader;
mod tile_source;

use coord::ScreenCoord;
use glutin::{ElementState, Event, MouseButton, MouseScrollDelta, VirtualKeyCode};
use map_view_gl::MapViewGl;
use tile_source::TileSource;
use std::time::{Duration, Instant};

#[cfg(target_os = "android")]
android_start!(main);

fn resize_callback(width: u32, height: u32) {
    println!("Window resized to {}x{}", width, height);
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Action {
    Nothing,
    Redraw,
    Close,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct InputState {
    mouse_position: (i32, i32),
    mouse_pressed: bool,
}

fn handle_event(event: Event, map: &mut MapViewGl, input_state: &mut InputState, sources: &mut TileSources) -> Action {
    match event {
        Event::Closed => Action::Close,
        Event::Awakened => Action::Redraw,
        Event::MouseInput(ElementState::Pressed, MouseButton::Left, position) => {
            input_state.mouse_pressed = true;
            if let Some(p) = position {
                input_state.mouse_position = p;
            }
            Action::Nothing
        },
        Event::MouseInput(ElementState::Released, MouseButton::Left, position) => {
            input_state.mouse_pressed = false;
            if let Some(p) = position {
                input_state.mouse_position = p;
            }
            Action::Nothing
        },
        Event::MouseMoved(x, y) => {
            if input_state.mouse_pressed {
                map.move_pixel(
                    f64::from(input_state.mouse_position.0 - x),
                    f64::from(input_state.mouse_position.1 - y),
                );
                input_state.mouse_position = (x, y);
                Action::Redraw
            } else {
                input_state.mouse_position = (x, y);
                Action::Nothing
            }
        },
        Event::MouseWheel(delta, _, position) => {
            let (dx, dy) = match delta {
                MouseScrollDelta::LineDelta(dx, dy) => {
                    // filter strange wheel events with huge values.
                    // (maybe this is just a personal touchpad driver issue)
                    if dx.abs() < 16.0 && dy.abs() < 16.0 {
                        (dx, dy * 10.0)
                    } else {
                        (0.0, 0.0)
                    }
                },
                MouseScrollDelta::PixelDelta(dx, dy) => (dx, dy),
            };
            if let Some(p) = position {
                input_state.mouse_position = p;
            }
            //TODO option to move or zoom on mouse wheel event
            //map.move_pixel(-dx as f64, -dy as f64);

            map.zoom_at(
                ScreenCoord::new(
                    f64::from(input_state.mouse_position.0),
                    f64::from(input_state.mouse_position.1),
                ),
                f64::from(dy) * 0.0125,
            );
            Action::Redraw
        },
        Event::KeyboardInput(glutin::ElementState::Pressed, _, Some(keycode)) => {
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
                    map.zoom(0.25);
                    Action::Redraw
                },
                VirtualKeyCode::Subtract => {
                    map.zoom(-0.25);
                    Action::Redraw
                },
                _ => Action::Nothing,
            }
        },
        Event::Refresh => {
            Action::Redraw
        },
        Event::Resized(w, h) => {
            map.set_viewport_size(w, h);
            Action::Redraw
        },
        _ => Action::Nothing,
    }
}

fn main() {
    let config = config::Config::from_toml("deltamap.toml").unwrap();
    let mut sources = TileSources::new(config.tile_sources()).unwrap();

    let mut window = glutin::WindowBuilder::new().build().unwrap();
    window.set_title("DeltaMap");
    window.set_window_resize_callback(Some(resize_callback as fn(u32, u32)));
    let _ = unsafe { window.make_current() };

    let proxy = window.create_window_proxy();

    let cx = context::Context::from_window(&window);
    let mut map = map_view_gl::MapViewGl::new(
        &cx,
        window.get_inner_size_pixels().unwrap(),
        move || { proxy.wakeup_event_loop(); },
    );

    let mut input_state = InputState {
        mouse_position: (0, 0),
        mouse_pressed: false,
    };

    let milli16 = Duration::from_millis(16);
    let mut draw_dur = Duration::from_millis(8);
    let mut last_draw = Instant::now();

    'outer: for event in window.wait_events() {
        let mut start_loop = Instant::now();

        let mut redraw = false;

        match handle_event(event, &mut map, &mut input_state, &mut sources) {
            Action::Close => break 'outer,
            Action::Redraw => {
                redraw = true;
            },
            Action::Nothing => {},
        }

        for event in window.poll_events() {
            match handle_event(event, &mut map, &mut input_state, &mut sources) {
                Action::Close => break 'outer,
                Action::Redraw => {
                    redraw = true;
                },
                Action::Nothing => {},
            }
        }

        {
            let diff = last_draw.elapsed();
            if diff + draw_dur * 2 < milli16 {
                if let Some(dur) = milli16.checked_sub(draw_dur * 2) {
                    std::thread::sleep(dur);
                    println!("SLEEP {}", dur.as_secs() as f64 + f64::from(dur.subsec_nanos()) * 1e-9);

                    for event in window.poll_events() {
                        match handle_event(event, &mut map, &mut input_state, &mut sources) {
                            Action::Close => break 'outer,
                            Action::Redraw => {
                                redraw = true;
                            },
                            Action::Nothing => {},
                        }
                    }
                }
            }
        }

        if redraw {
            let draw_start = Instant::now();
            map.draw(sources.current());
            draw_dur = draw_start.elapsed();

            let _ = window.swap_buffers();

            last_draw = Instant::now();

            let diff = start_loop.elapsed();
            println!("EVENT LOOP SECS {}", diff.as_secs() as f64 + f64::from(diff.subsec_nanos()) * 1e-9);
        }
    }
}

struct TileSources {
    current_index: usize,
    sources: Vec<(String, TileSource)>,
}

impl TileSources {
    pub fn new(sources: Vec<(String, TileSource)>) -> Option<TileSources> {
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

    pub fn switch_to_next(&mut self) {
        self.current_index = (self.current_index + 1) % self.sources.len();
    }

    pub fn switch_to_prev(&mut self) {
        self.current_index = (self.current_index + self.sources.len().saturating_sub(1)) % self.sources.len();
    }
}
