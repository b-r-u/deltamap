use coord::LatLonDeg;
use osmpbf::{Blob, BlobDecode, BlobReader, PrimitiveBlock};
use query::{find_query_matches, QueryArgs, QueryKind};
use scoped_threadpool::Pool;
use std::collections::hash_set::HashSet;
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
    query_args: QueryArgs,
    found_func: F,
    finished_func: G,
) -> Result<thread::JoinHandle<()>, String>
where P: AsRef<Path>,
      F: Fn(Vec<LatLonDeg>) -> ControlFlow + Send + 'static,
      G: Fn(Result<(), String>) + Send + 'static,
{
    let pbf_path = PathBuf::from(pbf_path.as_ref());
    let handle = thread::spawn(move|| {
        let res = par_search_blocking(pbf_path, query_args, found_func);
        finished_func(res);
    });

    Ok(handle)
}

pub fn par_search_blocking<P, F>(
    pbf_path: P,
    query_args: QueryArgs,
    found_func: F,
) -> Result<(), String>
where P: AsRef<Path>,
      F: Fn(Vec<LatLonDeg>) -> ControlFlow + Send + 'static,
{
    let query = query_args.compile()?;
    let query = &query;

    let first_pass = move |block: &PrimitiveBlock, _: &()| {
        let mut matches = vec![];
        let mut way_node_ids = vec![];

        match query {
            &QueryKind::ValuePattern(ref query) => {
                find_query_matches(block, query, &mut matches, &mut way_node_ids);
            },
            &QueryKind::KeyValue(ref query) => {
                find_query_matches(block, query, &mut matches, &mut way_node_ids);
            },
            _ => {
                //TODO implement
                unimplemented!();
            },
        }

        (matches, way_node_ids)
    };

    let mut way_node_ids: HashSet<i64> = HashSet::new();

    par_iter_blobs(
        &pbf_path,
        || {},
        first_pass,
        |(matches, node_ids)| {
            way_node_ids.extend(&node_ids);
            found_func(matches)
        },
    )?;

    let way_node_ids = &way_node_ids;

    let second_pass = move |block: &PrimitiveBlock, _: &()| {
        let mut matches = vec![];

        for node in block.groups().flat_map(|g| g.nodes()) {
            if way_node_ids.contains(&node.id()) {
                let pos = LatLonDeg::new(node.lat(), node.lon());
                matches.push(pos);
            }
        }

        for node in block.groups().flat_map(|g| g.dense_nodes()) {
            if way_node_ids.contains(&node.id) {
                let pos = LatLonDeg::new(node.lat(), node.lon());
                matches.push(pos);
            }
        }

        matches
    };

    par_iter_blobs(
        &pbf_path,
        || {},
        second_pass,
        found_func,
    )
}

fn par_iter_blobs<P, D, R, IF, CF, RF>(
    pbf_path: P,
    init_func: IF,
    compute_func: CF,
    mut result_func: RF,
) -> Result<(), String>
where P: AsRef<Path>,
      IF: Fn() -> D,
      CF: Fn(&PrimitiveBlock, &D) -> R + Send + Sync,
      RF: FnMut(R) -> ControlFlow,
      R: Send,
      D: Send,
{
    let num_threads = ::num_cpus::get();
    let mut pool = Pool::new(num_threads as u32);

    pool.scoped(|scope| {
        let mut reader = BlobReader::from_path(&pbf_path)
            .map_err(|e| format!("{}", e))?;

        let mut chans = Vec::with_capacity(num_threads);
        let (result_tx, result_rx) = sync_channel::<(usize, Result<Option<R>, String>)>(0);

        for thread_id in 0..num_threads {
            let thread_data = init_func();
            let result_tx = result_tx.clone();

            let (request_tx, request_rx) = sync_channel::<WorkerMessage>(0);
            chans.push(request_tx);

            let compute = &compute_func;

            scope.execute(move || {
                for request in request_rx.iter() {
                    match request {
                        WorkerMessage::PleaseStop => return,
                        WorkerMessage::DoBlob(blob) => {
                            match blob.decode() {
                                Ok(BlobDecode::OsmData(block)) => {
                                    let result = compute(&block, &thread_data);
                                    if result_tx.send((thread_id, Ok(Some(result)))).is_err() {
                                        return;
                                    }
                                },
                                //TODO also include other blob types in compute function
                                Ok(_) => {
                                    if result_tx.send((thread_id, Ok(None))).is_err() {
                                        return;
                                    }
                                },
                                Err(err) => {
                                    let _ = result_tx.send((thread_id, Err(format!("{}", err))));
                                    return;
                                },
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
            match matches {
                Err(err) => return Err(err),
                Ok(Some(matches)) => {
                    if result_func(matches) == ControlFlow::Break {
                        break;
                    }
                },
                _ => {},
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
