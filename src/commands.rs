mod new;
use self::new::New;
use crate::config::Config;
use clap::Subcommand;

#[derive(Clone, Debug, Eq, PartialEq, Subcommand)]
pub enum Command {
    New(New),
}

impl Command {
    pub fn run(self, config: Config) -> anyhow::Result<()> {
        match self {
            Command::New(new) => new.run(config),
        }
    }
}
