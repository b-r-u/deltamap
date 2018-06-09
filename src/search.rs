use osmpbf::{Element, ElementReader};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::thread;
use coord::LatLon;


#[derive(Debug, Eq, PartialEq)]
pub enum ControlFlow {
    Continue,
    Break,
}

impl<T, E> From<Result<T, E>> for ControlFlow
{
    fn from(result: Result<T, E>) -> Self {
        match result {
            Ok(_) => ControlFlow::Continue,
            Err(_) => ControlFlow::Break,
        }
    }
}

//TODO Add callbacks for other events: search finished, on error, ...
pub fn search_pbf<P, F>(
    pbf_path: P,
    search_pattern: &str,
    update_func: F,
) -> Result<thread::JoinHandle<()>, String>
where P: AsRef<Path>,
      F: Fn(LatLon) -> ControlFlow + Send + 'static,
{
    let pathbuf = PathBuf::from(pbf_path.as_ref());
    let re = Regex::new(search_pattern)
        .map_err(|e| format!("{}", e))?;
    let reader = ElementReader::from_path(&pathbuf)
        .map_err(|e| format!("Failed to read PBF file {:?}: {}", pbf_path.as_ref(), e))?;

    let handle = thread::spawn(move|| {
        reader.for_each(|element| {
            match element {
                Element::Node(node) => {
                    for (_key, val) in node.tags() {
                        if re.is_match(val) {
                            let pos = LatLon::new(node.lat(), node.lon());
                            if update_func(pos) == ControlFlow::Break {
                                return;
                            }
                            break;
                        }
                    }
                },
                Element::DenseNode(node) => {
                    for (_key, val) in node.tags() {
                        if re.is_match(val) {
                            let pos = LatLon::new(node.lat(), node.lon());
                            if update_func(pos) == ControlFlow::Break {
                                return;
                            }
                            break;
                        }
                    }
                },
                _ => {},
            }
        }).unwrap();
    });

    Ok(handle)
}
