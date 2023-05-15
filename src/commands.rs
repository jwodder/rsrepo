mod mkgithub;
mod new;
mod release;
mod set_msrv;
use self::mkgithub::Mkgithub;
use self::new::New;
use self::release::Release;
use self::set_msrv::SetMsrv;
use crate::config::Config;
use clap::Subcommand;

#[derive(Clone, Debug, Eq, PartialEq, Subcommand)]
pub enum Command {
    New(New),
    Mkgithub(Mkgithub),
    Release(Release),
    SetMsrv(SetMsrv),
}

impl Command {
    pub fn run(self, config: Config) -> anyhow::Result<()> {
        match self {
            Command::New(new) => new.run(config),
            Command::Mkgithub(mg) => mg.run(config),
            Command::Release(r) => r.run(config),
            Command::SetMsrv(sm) => sm.run(config),
        }
    }
}
