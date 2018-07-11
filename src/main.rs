extern crate cgmath;
#[macro_use]
extern crate clap;
extern crate directories;
extern crate env_logger;
extern crate glutin;
extern crate image;
#[macro_use]
extern crate lazy_static;
extern crate linked_hash_map;
#[macro_use]
extern crate log;
extern crate num_cpus;
extern crate osmpbf;
extern crate regex;
extern crate reqwest;
extern crate scoped_threadpool;
extern crate toml;

#[macro_use]
pub mod context;

pub mod args;
pub mod atmos_layer;
pub mod buffer;
pub mod config;
pub mod coord;
pub mod map_view;
pub mod map_view_gl;
pub mod marker_layer;
pub mod mercator_tile_layer;
pub mod mercator_view;
pub mod ortho_tile_layer;
pub mod orthografic_view;
pub mod program;
pub mod projection;
pub mod search;
pub mod session;
pub mod texture;
pub mod tile;
pub mod tile_atlas;
pub mod tile_cache;
pub mod tile_loader;
pub mod tile_source;
pub mod url_template;
pub mod vertex_attrib;

use coord::{LatLonDeg, ScreenCoord};
use glutin::dpi::{LogicalPosition, LogicalSize, PhysicalPosition};
use glutin::{ControlFlow, ElementState, Event, GlContext, MouseButton, MouseScrollDelta, VirtualKeyCode, WindowEvent};
use map_view_gl::MapViewGl;
use std::error::Error;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use tile_source::TileSource;


#[derive(Copy, Clone, Debug, PartialEq)]
enum Action {
    Nothing,
    Redraw,
    Resize(LogicalSize),
    Close,
}

impl Action {
    fn combine_with(&mut self, newer_action: Self) {
        *self = match (*self, newer_action) {
            (Action::Close, _) | (_, Action::Close) => Action::Close,
            (Action::Resize(..), Action::Resize(size)) => Action::Resize(size),
            (Action::Resize(size), _) | (_, Action::Resize(size)) => Action::Resize(size),
            (Action::Redraw, _) | (_, Action::Redraw) => Action::Redraw,
            (Action::Nothing, Action::Nothing) => Action::Nothing,
        };
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
struct InputState {
    mouse_position: LogicalPosition,
    mouse_pressed: bool,
    viewport_size: LogicalSize,
    dpi_factor: f64,
}

fn handle_event(
    event: &Event,
    map: &mut MapViewGl,
    input_state: &mut InputState,
    sources: &mut TileSources,
    marker_rx: &mpsc::Receiver<Vec<LatLonDeg>>,
) -> Action {
    match *event {
        Event::Awakened => {
            for pos in marker_rx.try_iter().flat_map(|c| c.into_iter()) {
                map.add_marker(pos.into());
            }
            Action::Redraw
        },
        Event::WindowEvent{ref event, ..} => match *event {
            WindowEvent::CloseRequested => Action::Close,
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => {
                input_state.mouse_pressed = true;
                Action::Nothing
            },
            WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Left, .. } => {
                input_state.mouse_pressed = false;
                Action::Nothing
            },
            WindowEvent::CursorMoved { position: pos, .. } => {
                if input_state.mouse_pressed {
                    map.move_pixel(
                        (input_state.mouse_position.x - pos.x) * input_state.dpi_factor,
                        (input_state.mouse_position.y - pos.y) * input_state.dpi_factor,
                    );
                    input_state.mouse_position = pos;
                    Action::Redraw
                } else {
                    input_state.mouse_position = pos;
                    Action::Nothing
                }
            },
            WindowEvent::MouseWheel { delta, modifiers, .. } => {
                let delta: PhysicalPosition = match delta {
                    MouseScrollDelta::LineDelta(dx, dy) => {
                        // filter strange wheel events with huge values.
                        // (maybe this is just a personal touchpad driver issue)
                        if dx.abs() < 16.0 && dy.abs() < 16.0 {
                            //TODO find a sensible line height value (servo (the glutin port) uses 38)
                            PhysicalPosition::new(
                                f64::from(dx) * input_state.dpi_factor,
                                f64::from(dy) * 38.0 * input_state.dpi_factor,
                            )
                        } else {
                            PhysicalPosition::new(0.0, 0.0)
                        }
                    },
                    MouseScrollDelta::PixelDelta(size) => {
                        size.to_physical(input_state.dpi_factor)
                    }
                };

                //TODO add option for default mouse wheel behavior (scroll or zoom?)
                //TODO add option to reverse scroll/zoom direction

                if modifiers.ctrl {
                    map.move_pixel(-delta.x, -delta.y);
                } else {
                    map.zoom_at(
                        ScreenCoord::new(
                            input_state.mouse_position.x * input_state.dpi_factor,
                            input_state.mouse_position.y * input_state.dpi_factor,
                        ),
                        delta.y * (1.0 / 320.0),
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
                            map.step_zoom(1, 1.0);
                        }
                        Action::Redraw
                    },
                    VirtualKeyCode::Subtract => {
                        if modifiers.ctrl {
                            map.change_tile_zoom_offset(-1.0);
                        } else {
                            map.step_zoom(-1, 1.0);
                        }
                        Action::Redraw
                    },
                    VirtualKeyCode::G => {
                        if modifiers.ctrl {
                            map.toggle_projection();
                            Action::Redraw
                        } else {
                            Action::Nothing
                        }
                    },
                    VirtualKeyCode::H => {
                        if modifiers.ctrl {
                            map.toggle_atmosphere();
                            Action::Redraw
                        } else {
                            Action::Nothing
                        }
                    },
                    _ => Action::Nothing,
                }
            },
            WindowEvent::Refresh => {
                Action::Redraw
            },
            WindowEvent::Resized(size) => {
                input_state.viewport_size = size;
                Action::Resize(size)
            },
            WindowEvent::HiDpiFactorChanged(dpi_factor) => {
                input_state.dpi_factor = dpi_factor;
                map.set_dpi_factor(dpi_factor);
                Action::Resize(input_state.viewport_size)
            }
            _ => Action::Nothing,
        },
        _ => Action::Nothing,
    }
}

fn dur_to_sec(dur: Duration) -> f64 {
    dur.as_secs() as f64 + f64::from(dur.subsec_nanos()) * 1e-9
}

fn run() -> Result<(), Box<Error>> {
    let config = {
        let arg_matches = args::parse();
        let config = config::Config::from_arg_matches(&arg_matches)?;
        if arg_matches.is_present("list-paths") {
            config.list_paths();
            return Ok(());
        }
        config
    };

    let mut sources = TileSources::new(config.tile_sources())
        .ok_or_else(|| "no tile sources provided.")?;

    let last_session = if config.open_last_session() {
        config::read_last_session().ok()
    } else {
        None
    };

    if let Some(tile_source) = last_session.as_ref().and_then(|s| s.tile_source.as_ref()) {
        sources.switch_to_name(tile_source);
    }

    let mut events_loop = glutin::EventsLoop::new();
    let builder = glutin::WindowBuilder::new()
        .with_title(format!("DeltaMap - {}", sources.current_name()));

    let gl_context = glutin::ContextBuilder::new();
    let gl_window = glutin::GlWindow::new(builder, gl_context, &events_loop)?;
    let window = gl_window.window();

    let _ = unsafe { gl_window.make_current() };
    let mut cx = context::Context::from_gl_window(&gl_window);

    let mut input_state = InputState {
        mouse_position: LogicalPosition::new(0.0, 0.0),
        mouse_pressed: false,
        viewport_size: window.get_inner_size().unwrap(),
        dpi_factor: window.get_hidpi_factor(),
    };

    let mut map = {
        let proxy = events_loop.create_proxy();

        map_view_gl::MapViewGl::new(
            &mut cx,
            input_state.viewport_size.to_physical(input_state.dpi_factor).into(),
            input_state.dpi_factor,
            move || { proxy.wakeup().unwrap(); },
            config.use_network(),
            config.async(),
        )
    };

    if let Some(ref session) = last_session {
        map.restore_session(session);
    }

    let (marker_tx, marker_rx) = mpsc::channel();
    if let (Some(path), Some(pattern)) = (config.pbf_path(), config.search_pattern()) {
        let proxy = events_loop.create_proxy();

        search::par_search(
            path,
            pattern,
            move |coords| {
                if coords.is_empty() {
                    search::ControlFlow::Continue
                } else {
                    if marker_tx.send(coords).is_err() {
                        return search::ControlFlow::Break;
                    }
                    proxy.wakeup().into()
                }
            },
            move |result| {
                if let Err(err) = result {
                    println!("search error: {}", err);
                } else {
                    info!("finished searching");
                }
            },
        )?;
    }

    let duration_per_frame = Duration::from_millis((1000.0 / config.fps() - 0.5).max(0.0).floor() as u64);
    info!("milliseconds per frame: {}", dur_to_sec(duration_per_frame) * 1000.0);

    // estimated draw duration
    let mut est_draw_dur = duration_per_frame;
    let mut last_draw = Instant::now();
    let mut increase_atlas_size_possible = true;

    loop {
        let start_source_id = sources.current().id();
        let mut action = Action::Nothing;

        events_loop.run_forever(|event| {
            let a = handle_event(&event, &mut map, &mut input_state, &mut sources, &marker_rx);
            action.combine_with(a);
            ControlFlow::Break
        });

        if action == Action::Close {
            break;
        }

        events_loop.poll_events(|event| {
            let a = handle_event(&event, &mut map, &mut input_state, &mut sources, &marker_rx);
            action.combine_with(a);
            if action == Action::Close {
                return;
            }
        });

        if action == Action::Close {
            break;
        }

        {
            let diff = last_draw.elapsed();
            if diff + est_draw_dur * 2 < duration_per_frame {
                if let Some(dur) = duration_per_frame.checked_sub(est_draw_dur * 2) {
                    std::thread::sleep(dur);

                    events_loop.poll_events(|event| {
                        let a = handle_event(&event, &mut map, &mut input_state, &mut sources, &marker_rx);
                        action.combine_with(a);
                        if action == Action::Close {
                            return;
                        }
                    });

                    if action == Action::Close {
                        break;
                    }
                }
            }
        }

        if let Action::Resize(size) = action {
            let phys_size = size.to_physical(input_state.dpi_factor);
            gl_window.resize(phys_size);

            let phys_size: (u32, u32) = phys_size.into();
            map.set_viewport_size(&mut cx, phys_size.0, phys_size.1);
        }

        let redraw = match action {
            Action::Redraw => true,
            Action::Resize(..) => true,
            _ => false,
        };

        if redraw {
            let draw_start = Instant::now();

            if !map.map_covers_viewport() {
                cx.clear_color((0.2, 0.2, 0.2, 1.0));
            }
            let draw_result = map.draw(&mut cx, sources.current());

            let draw_dur = draw_start.elapsed();


            let _ = gl_window.swap_buffers();

            last_draw = Instant::now();

            //TODO increase atlas size earlier to avoid excessive copying to the GPU
            //TODO increase max tile cache size?
            if increase_atlas_size_possible {
                let draws = match draw_result {
                    Ok(x) => x,
                    Err(x) => x,
                };
                if draws > 1 {
                    increase_atlas_size_possible = map.increase_atlas_size(&mut cx).is_ok();
                }
            }

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

    if config.open_last_session() {
        let mut session = map.to_session();
        session.tile_source = Some(sources.current_name().to_string());
        config::save_session(&session)?;
    }

    Ok(())
}

fn main() {
    env_logger::init();

    if let Err(err) = run() {
        println!("{}", err);
        std::process::exit(1);
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
                sources,
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

    pub fn switch_to_name(&mut self, name: &str) {
        for (index, &(ref n, _)) in self.sources.iter().enumerate() {
            if n == name {
                self.current_index = index;
                break;
            }
        }
    }
}
