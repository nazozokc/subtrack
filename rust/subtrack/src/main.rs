fn main() {
    std::process::exit(subtrack::run(std::env::args_os().skip(1).collect()))
}
