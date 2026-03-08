use gatebound_lab::{parse_args, run_lab};

fn main() {
    let spec = match parse_args(std::env::args()) {
        Ok(spec) => spec,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };

    if let Err(error) = run_lab(&spec) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
