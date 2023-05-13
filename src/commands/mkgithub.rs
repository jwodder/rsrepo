use crate::config::Config;
use crate::github::{GitHub, Label, NewRepoConfig, Topic};
use crate::package::Package;
use crate::readme::Repostatus;
use anyhow::bail;
use clap::Args;
use ghrepo::GHRepo;

/// Create a GitHub repository for the project and push
#[derive(Args, Clone, Debug, Eq, PartialEq)]
pub struct Mkgithub {
    /// Make the new repository private
    #[clap(short = 'P', long)]
    private: bool,

    /// Name for the repository
    ///
    /// If not specified, defaults to the name used in the `repository` URL in
    /// the Cargo metadata, or to the name of the package.
    #[clap(value_name = "NAME")]
    repo_name: Option<String>,
}

impl Mkgithub {
    pub fn run(self, config: Config) -> anyhow::Result<()> {
        let package = Package::locate()?;
        let metadata = package.metadata()?;
        let name = if let Some(s) = self.repo_name {
            s
        } else {
            match metadata.repository.as_ref().map(|s| s.parse::<GHRepo>()) {
                Some(Ok(r)) => {
                    if r.owner() != config.github_user {
                        bail!(
                            "Package repository URL does not belong to github-user set in config"
                        );
                    }
                    r.name().to_string()
                }
                Some(Err(_)) => bail!("Package repository URL does not point to GitHub"),
                None => metadata.name,
            }
        };

        let mut topics = Vec::from([Topic::new("rust")]);
        for keyword in metadata.keywords {
            let tp = Topic::new(&keyword);
            if tp != keyword {
                log::warn!("Keyword {keyword:?} sanitized to \"{tp}\" for use as GitHub topic");
            }
            topics.push(tp);
        }
        if package.readme().get()?.and_then(|r| r.repostatus()) == Some(Repostatus::Wip) {
            topics.push(Topic::new("work-in-progress"));
        }

        let mut repo_cfg = NewRepoConfig::new(name)
            .private(self.private)
            .topics(topics);
        if let Some(s) = metadata.description {
            repo_cfg = repo_cfg.description(s);
        }
        let github = GitHub::authed()?;
        let r = github.create_repository(repo_cfg)?;
        log::info!("Created GitHub repository {}", r.html_url);

        log::info!("Setting remote and pushing");
        let git = package.git();
        if git.remotes()?.contains("origin") {
            git.rm_remote("origin")?;
        }
        git.add_remote("origin", &r.ssh_url)?;
        git.run("push", ["-u", "origin", "refs/heads/*", "refs/tags/*"])?;

        log::info!("Creating Dependabot labels ...");
        let ghrepo = r.ghrepo()?;
        github.create_label(
            &ghrepo,
            Label::new(
                "dependencies",
                "8732bc",
                "Update one or more dependencies' versions",
            ),
        )?;
        github.create_label(
            &ghrepo,
            Label::new("d:cargo", "dea584", "Update a Cargo (Rust) dependency"),
        )?;
        github.create_label(
            &ghrepo,
            Label::new(
                "d:github-actions",
                "74fa75",
                "Update a GitHub Actions action dependency",
            ),
        )?;

        if metadata.repository.is_none() {
            log::info!("Setting 'package.repository' key in Cargo.toml ...");
            let manifest = package.manifest();
            let Some(mut doc) = manifest.get()? else {
                bail!("Cargo.toml suddenly disappeared!");
            };
            let Some(pkg) = doc.get_mut("package").and_then(|it| it.as_table_like_mut()) else {
                bail!("No [package] table in Cargo.toml");
            };
            pkg.insert("repository", toml_edit::value(r.html_url));
            manifest.set(doc)?;
        }

        Ok(())
    }
}
