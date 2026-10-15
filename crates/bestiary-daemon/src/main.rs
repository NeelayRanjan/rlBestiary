fn main() {
    let args: Vec<_> = std::env::args().collect();
    let Some(path) = args.get(1) else {
        eprintln!("usage: bestiary-daemon WORLD.db [UNTIL_TICK]");
        std::process::exit(2);
    };
    let until = args.get(2).map(|s| s.parse::<u64>()).transpose();
    let result = until
        .map_err(Box::<dyn std::error::Error + Send + Sync>::from)
        .and_then(|until| bestiary_daemon::run(std::path::Path::new(path), until, 100, None));
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
