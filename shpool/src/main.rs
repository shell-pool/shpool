use clap::Parser;

fn main() -> anyhow::Result<()> {
    libshpool::run(libshpool::Args::parse())
}
