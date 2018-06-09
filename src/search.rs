use regex::Regex;
use std::path::{Path, PathBuf};
use std::thread;
use osmpbf::{Element, ElementReader};


pub fn search_pbf<P, F>(
    pbf_path: P,
    search_pattern: &str,
    update_func: F,
) -> Result<thread::JoinHandle<()>, String>
where P: AsRef<Path>,
      F: Fn(f64, f64) + Send + 'static,
{
    let pathbuf = PathBuf::from(pbf_path.as_ref());
    let re = Regex::new(search_pattern)
        .map_err(|e| format!("{}", e))?;
    let reader = ElementReader::from_path(&pathbuf)
        .map_err(|e| format!("Failed to read PBF file {:?}: {}", pbf_path.as_ref(), e))?;

    let handle = thread::spawn(move|| {
        //TODO do something about the unwrap()
        reader.for_each(|element| {
            match element {
                Element::Node(node) => {
                    for (_key, val) in node.tags() {
                        if re.is_match(val) {
                            update_func(node.lat(), node.lon());
                            break;
                        }
                    }
                },
                Element::DenseNode(dnode) => {
                    for (_key, val) in dnode.tags() {
                        if re.is_match(val) {
                            update_func(dnode.lat(), dnode.lon());
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
