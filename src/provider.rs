use crate::config::Config;
use crate::github::GitHub;
use once_cell::unsync::OnceCell;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub(crate) struct Provider {
    config_path: Option<PathBuf>,
    // We need to use OnceCell instead of plain Options so that multiple
    // methods can be called on a Provider without causing a compilation error
    // due to multiple mutable borrows.  In addition, we need to use the
    // OnceCell from once_cell because the std OnceCell's get_or_try_init() is
    // still unstable.
    config: OnceCell<Config>,
    github: OnceCell<GitHub>,
}

impl Provider {
    pub(crate) fn new(config_path: Option<PathBuf>) -> Provider {
        Provider {
            config_path,
            config: OnceCell::new(),
            github: OnceCell::new(),
        }
    }

    pub(crate) fn config(&self) -> anyhow::Result<&Config> {
        self.config
            .get_or_try_init(|| Config::load(self.config_path.as_deref()))
    }

    pub(crate) fn github(&self) -> anyhow::Result<&GitHub> {
        self.github.get_or_try_init(GitHub::authed)
    }
}
