pub fn init_cli() -> clap::ArgMatches {
    let parser = clap::Command::new(env!("CARGO_BIN_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .about("Collects all dependencies of an executable or a dynamic library on MacOS and bundles them for portability.");

    let parser = parser.arg(
        clap::Arg::new("BINARY_PATH")
            .short('i')
            .long("input")
            .required(true)
            .help("Path of the binary\ndesired to be bundled."),
    );
    let parser = parser.arg(
        clap::Arg::new("BUNDLE_PATH")
            .short('o')
            .long("output")
            .required(true)
            .help("Path of the destination folder"),
    );
    let parser = parser.arg(
        clap::Arg::new("LOG_LEVEL")
            .short('l')
            .long("log")
            .default_value("INFO")
            .help("[TRACE, INFO, DEBUG, WARNING, ERROR]"),
    );

    parser.get_matches()
}
