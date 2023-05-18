mod mkgithub;
mod new;
mod release;
mod set_msrv;
use self::mkgithub::Mkgithub;
use self::new::New;
use self::release::Release;
use self::set_msrv::SetMsrv;
use clap::Subcommand;
use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq, Subcommand)]
pub enum Command {
    New(New),
    Mkgithub(Mkgithub),
    Release(Release),
    SetMsrv(SetMsrv),
}

impl Command {
    pub fn run(self, config_path: Option<PathBuf>) -> anyhow::Result<()> {
        match self {
            Command::New(new) => new.run(config_path),
            Command::Mkgithub(mg) => mg.run(config_path),
            Command::Release(r) => r.run(config_path),
            Command::SetMsrv(sm) => sm.run(config_path),
        }
    }
}
