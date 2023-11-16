use crate::github::{CreateRepoBody, Label, Topic};
use crate::package::Package;
use crate::provider::Provider;
use crate::readme::Repostatus;
use anyhow::bail;
use clap::Args;
use ghrepo::GHRepo;
use std::borrow::Cow;

/// Create a GitHub repository for the project and push
#[derive(Args, Clone, Debug, Eq, PartialEq)]
pub(crate) struct Mkgithub {
    /// Make the new repository private
    #[arg(short = 'P', long)]
    private: bool,

    /// Name for the repository
    ///
    /// If not specified, defaults to the name used in the `repository` URL in
    /// the Cargo metadata, or to the name of the package.
    #[arg(value_name = "NAME")]
    repo_name: Option<String>,
}

impl Mkgithub {
    pub(crate) fn run(self, provider: Provider) -> anyhow::Result<()> {
        let github = provider.github()?;
        let package = Package::locate()?;
        let metadata = package.metadata()?;
        let name = if let Some(s) = self.repo_name {
            s
        } else {
            match metadata.repository.as_ref().map(|s| s.parse::<GHRepo>()) {
                Some(Ok(r)) => {
                    let github_user = match provider.config()?.github_user.as_ref() {
                        Some(user) => Cow::Borrowed(user),
                        None => Cow::Owned(github.whoami()?),
                    };
                    if r.owner() != *github_user {
                        bail!("Package repository URL does not belong to GitHub user");
                    }
                    r.name().to_string()
                }
                Some(Err(_)) => bail!("Package repository URL does not point to GitHub"),
                None => metadata.name,
            }
        };

        let mut new_repo = CreateRepoBody {
            name,
            description: None,
            private: Some(self.private),
            delete_branch_on_merge: Some(true),
        };
        if let d @ Some(_) = metadata.description {
            new_repo.description = d;
        }
        let repo = github.create_repository(new_repo)?;
        log::info!("Created GitHub repository {}", repo.html_url);

        log::info!("Setting remote and pushing");
        let git = package.git();
        if git.remotes()?.contains("origin") {
            git.rm_remote("origin")?;
        }
        git.add_remote("origin", &repo.ssh_url)?;
        git.run("push", ["-u", "origin", "refs/heads/*", "refs/tags/*"])?;

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
        log::info!(
            "Setting repository topics to: {}",
            itertools::join(&topics, " ")
        );
        github.set_topics(&repo, topics)?;

        log::info!("Creating Dependabot labels ...");
        github.create_label(
            &repo,
            Label::new(
                "dependencies",
                "8732bc",
                "Update one or more dependencies' versions",
            ),
        )?;
        github.create_label(
            &repo,
            Label::new("d:cargo", "dea584", "Update a Cargo (Rust) dependency"),
        )?;
        github.create_label(
            &repo,
            Label::new(
                "d:github-actions",
                "74fa75",
                "Update a GitHub Actions action dependency",
            ),
        )?;

        if metadata.repository.is_none() {
            log::info!("Setting 'package.repository' field in Cargo.toml ...");
            package.set_package_field("repository", repo.html_url)?;
        } else if metadata.repository != Some(repo.html_url) {
            log::warn!(
                "'package.repository' field in Cargo.toml differs from GitHub repository URL"
            );
        }

        Ok(())
    }
}
