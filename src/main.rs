extern crate clap;
extern crate glutin;
extern crate image;
extern crate linked_hash_map;
extern crate reqwest;
extern crate toml;
extern crate xdg;

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

use clap::Arg;
use coord::ScreenCoord;
use glutin::{ElementState, Event, MouseButton, MouseScrollDelta, VirtualKeyCode};
use map_view_gl::MapViewGl;
use std::time::{Duration, Instant};
use tile_source::TileSource;


static VERSION: &'static str = "0.1.0";


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

fn handle_event(event: &Event, map: &mut MapViewGl, input_state: &mut InputState, sources: &mut TileSources) -> Action {
    match *event {
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
                    map.step_zoom(1, 0.5);
                    Action::Redraw
                },
                VirtualKeyCode::Subtract => {
                    map.step_zoom(-1, 0.5);
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
    let matches = clap::App::new("DeltaMap")
        .version(VERSION)
        .author("Johannes Hofmann <mail@b-r-u.org>")
        .about("A map viewer")
        .arg(Arg::with_name("config")
            .short("c")
            .long("config")
            .value_name("FILE")
            .help("Set a custom config file")
            .takes_value(true))
        .get_matches();

    let config = if let Some(config_path) = matches.value_of_os("config") {
            config::Config::from_toml_file(config_path).unwrap()
        } else {
            config::Config::load().unwrap()
        };

    let mut sources = TileSources::new(config.tile_sources()).unwrap();

    let mut window = glutin::WindowBuilder::new().build().unwrap();
    window.set_title(&("DeltaMap - ".to_string() + sources.current_name()));

    //TODO Find a safe way to trigger a redraw from a resize callback.
    //TODO The callback is only allowed to access static content.
    window.set_window_resize_callback(None);

    let _ = unsafe { window.make_current() };
    let cx = context::Context::from_window(&window);

    let mut map = {
        let proxy = window.create_window_proxy();

        map_view_gl::MapViewGl::new(
            &cx,
            window.get_inner_size_pixels().unwrap(),
            move || { proxy.wakeup_event_loop(); },
        )
    };

    let mut input_state = InputState {
        mouse_position: (0, 0),
        mouse_pressed: false,
    };

    let milli16 = Duration::from_millis(16);
    let mut draw_dur = Duration::from_millis(8);
    let mut last_draw = Instant::now();

    'outer: for event in window.wait_events() {
        let start_source_id = sources.current().id();

        let mut redraw = false;

        match handle_event(&event, &mut map, &mut input_state, &mut sources) {
            Action::Close => break 'outer,
            Action::Redraw => {
                redraw = true;
            },
            Action::Nothing => {},
        }

        for event in window.poll_events() {
            match handle_event(&event, &mut map, &mut input_state, &mut sources) {
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

                    for event in window.poll_events() {
                        match handle_event(&event, &mut map, &mut input_state, &mut sources) {
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
        }

        // set window title
        if sources.current().id() != start_source_id {
            window.set_title(&("DeltaMap - ".to_string() + sources.current_name()));
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
