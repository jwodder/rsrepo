mod cmd;
mod project;
use crate::project::Project;
use clap::Parser;

#[derive(Debug, Eq, Parser, PartialEq)]
#[clap(version)]
struct Arguments {
    /// Set logging level
    #[clap(
        short,
        long,
        default_value = "INFO",
        value_name = "OFF|ERROR|WARN|INFO|DEBUG|TRACE"
    )]
    log_level: log::LevelFilter,
}

fn main() -> anyhow::Result<()> {
    let args = Arguments::parse();
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!("[{:<5}] {}", record.level(), message))
        })
        .level(args.log_level)
        .chain(std::io::stderr())
        .apply()
        .unwrap();
    let project = Project::locate()?;
    println!("{}", project.path().display());
    Ok(())
}
