use clap;
use clap::Arg;


pub fn parse<'a>() -> clap::ArgMatches<'a> {
    clap::App::new("DeltaMap")
        .version(crate_version!())
        .author(crate_authors!())
        .about(crate_description!())
        .arg(Arg::with_name("config")
            .short("c")
            .long("config")
            .value_name("FILE")
            .help("Set a custom config file")
            .takes_value(true))
        .arg(Arg::with_name("list-paths")
            .long("list-paths")
            .help("Print paths of configuration files and directories \
                and exit the program."))
        .arg(Arg::with_name("tile-sources")
            .short("t")
            .long("tile-sources")
            .value_name("FILE")
            .help("Set a custom tile sources file")
            .takes_value(true))
        .arg(Arg::with_name("pbf")
            .long("pbf")
            .value_name("FILE")
            .help("Set a *.osm.pbf file")
            .takes_value(true))
        .arg(Arg::with_name("search")
            .short("s")
            .long("search")
            .value_name("PATTERN")
            .help("Search for places which match the given pattern")
            .takes_value(true))
        .arg(Arg::with_name("keyval")
            .short("k")
            .long("keyval")
            .value_name("KEY:VALUE")
            .validator(|s| {
                let colons = (s.split(':').count() - 1).max(0);
                if colons == 1 {
                    Ok(())
                } else {
                    Err(format!("exactly one colon (':') is required to separate key and value, \
                                found {}", colons))
                }
            })
            .help("Search for places that are tagged with the given key and value")
            .takes_value(true))
        .arg(Arg::with_name("fps")
            .long("fps")
            .value_name("FPS")
            .validator(|s| {
                s.parse::<f64>()
                    .map(|_| ())
                    .map_err(|e| format!("{}", e))
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
                The UI is blocked while tiles are loading."))
        .get_matches()
}
