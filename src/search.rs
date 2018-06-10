use scoped_threadpool::Pool;
use coord::LatLon;
use osmpbf::{Blob, BlobDecode, BlobReader};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::mpsc::sync_channel;
use std::thread;


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

enum WorkerMessage {
    PleaseStop,
    DoBlob(Box<Blob>),
}

pub fn par_search<P, F, G>(
    pbf_path: P,
    search_pattern: &str,
    found_func: F,
    finished_func: G,
) -> Result<thread::JoinHandle<()>, String>
where P: AsRef<Path>,
      F: Fn(Vec<LatLon>) -> ControlFlow + Send + 'static,
      G: Fn(Result<(), String>) + Send + 'static,
{
    let pbf_path = PathBuf::from(pbf_path.as_ref());
    let search_pattern = search_pattern.to_string();
    let handle = thread::spawn(move|| {
        let res = par_search_blocking(pbf_path, &search_pattern, found_func);
        finished_func(res);
    });

    Ok(handle)
}

pub fn par_search_blocking<P, F>(
    pbf_path: P,
    search_pattern: &str,
    found_func: F,
) -> Result<(), String>
where P: AsRef<Path>,
      F: Fn(Vec<LatLon>) -> ControlFlow + Send + 'static,
{
    let num_threads = ::num_cpus::get();
    let mut pool = Pool::new(num_threads as u32);

    pool.scoped(|scope| {
        let re = Regex::new(search_pattern)
            .map_err(|e| format!("{}", e))?;
        let mut reader = BlobReader::from_path(&pbf_path)
            .map_err(|e| format!("{}", e))?;

        let mut chans = Vec::with_capacity(num_threads);
        let (result_tx, result_rx) = sync_channel::<(usize, Result<Vec<LatLon>, String>)>(0);

        for thread_id in 0..num_threads {
            let re = re.clone();
            let result_tx = result_tx.clone();

            let (request_tx, request_rx) = sync_channel::<WorkerMessage>(0);
            chans.push(request_tx);

            scope.execute(move || {
                for request in request_rx.iter() {
                    match request {
                        WorkerMessage::PleaseStop => return,
                        WorkerMessage::DoBlob(blob) => {
                            let mut matches = vec![];
                            let block = match blob.decode() {
                                Ok(b) => b,
                                Err(err) => {
                                    let _ = result_tx.send((thread_id, Err(format!("{}", err))));
                                    return;
                                }
                            };
                            if let BlobDecode::OsmData(block) = block {
                                for node in block.groups().flat_map(|g| g.nodes()) {
                                    for (_key, val) in node.tags() {
                                        if re.is_match(val) {
                                            let pos = LatLon::new(node.lat(), node.lon());
                                            matches.push(pos);
                                            break;
                                        }
                                    }
                                }

                                for node in block.groups().flat_map(|g| g.dense_nodes()) {
                                    for (_key, val) in node.tags() {
                                        if re.is_match(val) {
                                            let pos = LatLon::new(node.lat(), node.lon());
                                            matches.push(pos);
                                            break;
                                        }
                                    }
                                }
                            }
                            if result_tx.send((thread_id, Ok(matches))).is_err() {
                                return;
                            }
                        }
                    };
                }
            });
        }

        let mut stopped_threads = 0;

        // send initial message to each worker thread
        for channel in &chans {
            match reader.next() {
                Some(Ok(blob)) => {
                    channel.send(WorkerMessage::DoBlob(Box::new(blob)))
                        .map_err(|e| format!("{}", e))?;

                },
                Some(Err(err)) => {
                    return Err(format!("{}", err));
                },
                None => {
                    channel.send(WorkerMessage::PleaseStop)
                        .map_err(|e| format!("{}", e))?;
                    stopped_threads += 1;
                },
            }
        }

        if stopped_threads == num_threads {
            return Ok(());
        }

        for (thread_id, matches) in result_rx.iter() {
            let matches = matches?;

            if found_func(matches) == ControlFlow::Break {
                break;
            }

            match reader.next() {
                Some(Ok(blob)) => {
                    chans[thread_id].send(WorkerMessage::DoBlob(Box::new(blob)))
                        .map_err(|e| format!("{}", e))?;
                },
                Some(Err(err)) => {
                    return Err(format!("{}", err));
                },
                None => {
                    chans[thread_id].send(WorkerMessage::PleaseStop)
                        .map_err(|e| format!("{}", e))?;
                    stopped_threads += 1;
                    if stopped_threads == num_threads {
                        break;
                    }
                }
            }
        }

        Ok(())
    })
}
