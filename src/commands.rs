mod begin_dev;
mod inspect;
mod mkgithub;
mod new;
mod release;
mod set_msrv;
use self::begin_dev::BeginDev;
use self::inspect::Inspect;
use self::mkgithub::Mkgithub;
use self::new::New;
use self::release::Release;
use self::set_msrv::SetMsrv;
use crate::provider::Provider;
use clap::Subcommand;

#[derive(Clone, Debug, Eq, PartialEq, Subcommand)]
pub(crate) enum Command {
    New(New),
    BeginDev(BeginDev),
    Inspect(Inspect),
    Mkgithub(Mkgithub),
    Release(Release),
    SetMsrv(SetMsrv),
}

impl Command {
    pub(crate) fn run(self, provider: Provider) -> anyhow::Result<()> {
        match self {
            Command::New(new) => new.run(provider),
            Command::BeginDev(begin_dev) => begin_dev.run(provider),
            Command::Inspect(inspect) => inspect.run(provider),
            Command::Mkgithub(mg) => mg.run(provider),
            Command::Release(r) => r.run(provider),
            Command::SetMsrv(sm) => sm.run(provider),
        }
    }
}
