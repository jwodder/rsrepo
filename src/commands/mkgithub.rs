use crate::config::Config;
use crate::github::{GitHub, Label, NewRepoConfig, Topic};
use crate::project::Project;
use anyhow::bail;
use clap::Args;
use ghrepo::GHRepo;

#[derive(Args, Clone, Debug, Eq, PartialEq)]
pub struct Mkgithub {
    #[clap(short = 'P', long)]
    private: bool,

    repo_name: Option<String>,
}

impl Mkgithub {
    pub fn run(self, _: Config) -> anyhow::Result<()> {
        let project = Project::locate()?;
        let metadata = project.metadata()?;
        let name = if let Some(s) = self.repo_name {
            s
        } else {
            match metadata.repository.map(|s| s.parse::<GHRepo>()) {
                Some(Ok(r)) => r.name().to_string(),
                Some(Err(_)) => bail!("Project repository URL does not point to GitHub"),
                None => metadata.name,
            }
        };

        let mut topics = Vec::new();
        for keyword in metadata.keywords {
            let tp = Topic::new(&keyword);
            if tp != keyword {
                log::warn!("Keyword {keyword:?} sanitized to \"{tp}\" for use as GitHub topic");
            }
            topics.push(tp);
        }
        topics.push(Topic::new("rust"));
        // TODO: Add work-in-progress topic if README has WIP repostatus badge

        let mut repo_cfg = NewRepoConfig::new(&name)
            .private(self.private)
            .topics(topics);
        if let Some(s) = metadata.description {
            repo_cfg = repo_cfg.description(&s);
        }
        let github = GitHub::new()?;
        let r = github.create_repository(repo_cfg)?;
        log::info!("Created GitHub repository {}", r.html_url);

        log::info!("Setting remote and pushing");
        let git = project.git();
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
        Ok(())
    }
}
