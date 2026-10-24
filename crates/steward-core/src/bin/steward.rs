fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: steward <database-path> <method> [json-payload]");
        std::process::exit(2);
    }
    let app = steward_core::service::Service::new(&args[1]).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1)
    });
    let payload = args
        .get(3)
        .map(|s| serde_json::from_str(s))
        .transpose()
        .unwrap_or_else(|e| {
            eprintln!("{e}");
            std::process::exit(2)
        })
        .unwrap_or(serde_json::Value::Null);
    match app.call(&args[2], payload) {
        Ok(v) => println!("{}", serde_json::to_string_pretty(&v).unwrap()),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1)
        }
    }
}
