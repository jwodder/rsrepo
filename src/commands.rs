mod mkgithub;
mod new;
mod release;
use self::mkgithub::Mkgithub;
use self::new::New;
use self::release::Release;
use crate::config::Config;
use clap::Subcommand;

#[derive(Clone, Debug, Eq, PartialEq, Subcommand)]
pub enum Command {
    New(New),
    Mkgithub(Mkgithub),
    Release(Release),
}

impl Command {
    pub fn run(self, config: Config) -> anyhow::Result<()> {
        match self {
            Command::New(new) => new.run(config),
            Command::Mkgithub(mg) => mg.run(config),
            Command::Release(r) => r.run(config),
        }
    }
}
