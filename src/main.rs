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

use clap::Arg;
use coord::ScreenCoord;
use glutin::{ElementState, Event, MouseButton, MouseScrollDelta, VirtualKeyCode};
use map_view_gl::MapViewGl;
use std::error::Error;
use std::time::{Duration, Instant};
use tile_source::TileSource;


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
    lctrl_pressed: bool,
    rctrl_pressed: bool,
}

impl InputState {
    fn ctrl_pressed(&self) -> bool {
        self.lctrl_pressed | self.rctrl_pressed
    }
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
                        //TODO find a sensible line height value (servo (the glutin port) uses 38)
                        (dx, dy * 38.0)
                    } else {
                        (0.0, 0.0)
                    }
                },
                MouseScrollDelta::PixelDelta(dx, dy) => (dx, dy),
            };
            if let Some(p) = position {
                input_state.mouse_position = p;
            }

            //TODO add option for default mouse wheel behavior (scroll or zoom?)
            //TODO add option to reverse scroll/zoom direction

            if input_state.ctrl_pressed() {
                map.move_pixel(f64::from(-dx), f64::from(-dy));
            } else {
                map.zoom_at(
                    ScreenCoord::new(
                        f64::from(input_state.mouse_position.0),
                        f64::from(input_state.mouse_position.1),
                    ),
                    f64::from(dy) * (1.0 / 320.0),
                );
            }
            Action::Redraw
        },
        Event::KeyboardInput(glutin::ElementState::Pressed, _, Some(keycode)) => {
            match keycode {
                VirtualKeyCode::Escape => {
                    Action::Close
                },
                VirtualKeyCode::LControl => {
                    input_state.lctrl_pressed = true;
                    Action::Nothing
                },
                VirtualKeyCode::RControl => {
                    input_state.rctrl_pressed = true;
                    Action::Nothing
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
                    if input_state.ctrl_pressed() {
                        map.change_tile_zoom_offset(1.0);
                    } else {
                        map.step_zoom(1, 0.5);
                    }
                    Action::Redraw
                },
                VirtualKeyCode::Subtract => {
                    if input_state.ctrl_pressed() {
                        map.change_tile_zoom_offset(-1.0);
                    } else {
                        map.step_zoom(-1, 0.5);
                    }
                    Action::Redraw
                },
                _ => Action::Nothing,
            }
        },
        Event::KeyboardInput(glutin::ElementState::Released, _, Some(keycode)) => {
            match keycode {
                VirtualKeyCode::LControl => {
                    input_state.lctrl_pressed = false;
                    Action::Nothing
                },
                VirtualKeyCode::RControl => {
                    input_state.rctrl_pressed = false;
                    Action::Nothing
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

fn dur_to_sec(dur: Duration) -> f64 {
    dur.as_secs() as f64 + f64::from(dur.subsec_nanos()) * 1e-9
}

fn main() {
    env_logger::init();

    let matches = clap::App::new("DeltaMap")
        .version(crate_version!())
        .author(crate_authors!())
        .about(crate_description!())
        .arg(Arg::with_name("config")
            .short("c")
            .long("config")
            .value_name("FILE")
            .help("Set a custom config file")
            .takes_value(true))
        .arg(Arg::with_name("fps")
            .long("fps")
            .value_name("FPS")
            .validator(|s| {
                s.parse::<f64>()
                    .map(|_| ())
                    .map_err(|e| e.description().to_string())
            })
            .help("Set target frames per second (default is 60). \
                This should equal the refresh rate of the display.")
            .takes_value(true))
        .arg(Arg::with_name("offline")
            .long("offline")
            .help("Do not use the network. \
                Try to load tiles from the offline file system cache."))
        .arg(Arg::with_name("sync")
            .long("sync")
            .help("Load tiles in a synchronous fashion. \
                Interaction is not possible while tiles are loading."))
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
            !matches.is_present("offline"),
            !matches.is_present("sync"),
        )
    };

    let mut input_state = InputState {
        mouse_position: (0, 0),
        mouse_pressed: false,
        lctrl_pressed: false,
        rctrl_pressed: false,
    };

    let fps: f64 = matches.value_of("fps").map(|s| s.parse().unwrap()).unwrap_or_else(|| config.fps());
    let duration_per_frame = Duration::from_millis((1000.0 / fps - 0.5).max(0.0).floor() as u64);
    info!("milliseconds per frame: {}", dur_to_sec(duration_per_frame) * 1000.0);

    // estimated draw duration
    let mut est_draw_dur = duration_per_frame;
    let mut last_draw = Instant::now();
    let mut increase_atlas_size = true;

    'outer: for event in window.wait_events() {
        debug!("{:?}", &event);

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
            debug!("{:?}", &event);
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
            if diff + est_draw_dur * 2 < duration_per_frame {
                if let Some(dur) = duration_per_frame.checked_sub(est_draw_dur * 2) {
                    std::thread::sleep(dur);

                    for event in window.poll_events() {
                        debug!("after sleep {:?}", &event);
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
            let draw_result = map.draw(sources.current());
            let draw_dur = draw_start.elapsed();

            let _ = window.swap_buffers();

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
