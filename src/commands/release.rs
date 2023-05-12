use crate::changelog::{Changelog, ChangelogHeader, ChangelogSection};
use crate::cmd::LoggedCommand;
use crate::config::Config;
use crate::github::{CreateRelease, GitHub, Topic};
use crate::project::Project;
use crate::readme::{Badge, Repostatus};
use crate::util::{bump_version, move_dirtree_into, this_year, Bump};
use anyhow::{bail, Context};
use clap::Args;
use ghrepo::LocalRepo;
use renamore::rename_exclusive;
use semver::Version;
use std::collections::HashSet;
use std::fs::create_dir_all;
use std::io::{self, Write};
use tempfile::NamedTempFile;

#[derive(Args, Clone, Debug, Eq, PartialEq)]
#[group(multiple = false)]
pub struct BumpOptions {
    #[clap(long)]
    major: bool,
    #[clap(long)]
    minor: bool,
    #[clap(long)]
    patch: bool,
}

#[derive(Args, Clone, Debug, Eq, PartialEq)]
pub struct Release {
    #[command(flatten)]
    bump: BumpOptions,

    #[clap(value_parser = parse_v_version, conflicts_with = "bump")]
    version: Option<Version>,
}

impl Release {
    pub fn run(self, _: Config) -> anyhow::Result<()> {
        let project = Project::locate()?;
        let git = project.git();
        let github = GitHub::authed()?;

        let metadata = project.metadata()?;
        let old_version = metadata.version;
        let ghrepo = LocalRepo::new(project.path())
            .github_remote("origin")
            .context("Could not determine GitHub repository for local repository")?;
        let is_bin = project.is_bin()?;
        let is_lib = project.is_lib()?;
        let publish = metadata.publish.as_deref() != Some(&[]);

        // Determine new version
        let new_version = if let Some(v) = self.version {
            // Overrides all checks
            v
        } else {
            let tag_version = project.latest_tag_version()?;
            if let Some(tag_version) = tag_version.as_ref() {
                // If the latest tag is not equal to the manifest version
                // (equality only allowed when bumping; enforced by the check
                // for tag existence below) and the manifest version is not a
                // greater prerelease, error.
                // TODO: Unit-test this to confirm I like the logic.
                if tag_version >= &old_version || old_version.pre.is_empty() {
                    // TODO: Improve error message
                    bail!("Latest Git-tagged version exceeds manifest version");
                }
            }
            if let Some(bump) = self.bump() {
                if let Some(tag_version) = tag_version {
                    if !tag_version.pre.is_empty() {
                        bail!("Latest Git tag is a prerelease; cannot bump");
                    }
                    bump_version(tag_version, bump)
                } else {
                    bail!("No Git tag to bump");
                }
            } else {
                // Strip any pre-release segment
                Version::new(old_version.major, old_version.minor, old_version.patch)
            }
        };
        if git.tag_exists(&new_version.to_string())?
            || git.tag_exists(&format!("v{new_version}"))?
        {
            bail!("New version v{new_version} already tagged");
        }

        log::info!("Preparing version {new_version} ...");

        if new_version != old_version {
            log::info!("Setting version in Cargo.toml ...");
            project.set_cargo_version(new_version.clone())?;
        }

        if is_bin && project.path().join("Cargo.lock").exists() {
            log::info!("Setting version in Cargo.lock ...");
            LoggedCommand::new("cargo")
                .arg("update")
                .arg("-p")
                .arg(&metadata.name)
                .arg("--precise")
                .arg(new_version.to_string())
                .arg("--offline")
                .current_dir(project.path())
                .status()?;
        }

        let release_date = chrono::Local::now().date_naive();
        let chlog_content;
        if let Some(mut chlog) = project.changelog()? {
            log::info!("Updating CHANGELOG.md ...");
            if let Some(most_recent) = chlog.sections.iter_mut().next() {
                match most_recent.header {
                    ChangelogHeader::Released { .. } => bail!("No changelog section to update"),
                    _ => {
                        most_recent.header = ChangelogHeader::Released {
                            version: new_version.clone(),
                            date: release_date,
                        }
                    }
                }
                chlog_content = Some(most_recent.content.clone());
            } else {
                bail!("No changelog section to update");
            }
            project.set_changelog(chlog)?;
        } else {
            chlog_content = None;
        };

        let Some(mut readme) = project.readme()? else {
            bail!("Project lacks README.md");
        };
        let mut changed = false;
        let activated = if readme.repostatus() == Some(Repostatus::Wip) {
            log::info!("Setting repostatus to Active ...");
            readme.set_repostatus_badge(Badge {
                alt: "Project Status: Active – The project has reached a stable, usable state and is being actively developed.".into(),
                url: "https://www.repostatus.org/badges/latest/active.svg".into(),
                target: "https://www.repostatus.org/#active".into(),
            });
            changed = true;
            true
        } else {
            false
        };
        if readme.ensure_crates_links(&metadata.name, is_lib) {
            log::info!("Adding crates.io links to README.md ...");
            changed = true;
        }
        if changed {
            project.set_readme(readme)?;
        }

        log::info!("Updating copyright years in LICENSE ...");
        let mut years = git.commit_years()?;
        years.insert(this_year());
        project.update_license_years(years)?;

        log::info!("Committing ...");
        {
            let mut template = NamedTempFile::new().context("could not create temporary file")?;
            write_commit_template(template.as_file_mut(), &new_version, chlog_content)
                .context("error writing to commit message template")?;
            git.command()
                .arg("commit")
                .arg("-a")
                .arg("-v")
                .arg("--template")
                .arg(template.path())
                .status()
                .context("Commit cancelled; aborting")?;
        }

        log::info!("Tagging ...");
        let tag_name = format!("v{new_version}");
        git.command()
            .arg("tag")
            .arg("-s")
            .arg("-m")
            .arg(format!("Version {new_version}"))
            .arg(&tag_name)
            .status()?;

        // Publish (skip if `publish = false`)
        if publish {
            let toplevel = git
                .toplevel()
                .context("Could not determine root of Git repository")?;
            let stash_name = match toplevel.file_name() {
                Some(s) => format!("{}.stash", s.to_string_lossy()),
                None => bail!("Cannot calculate sibling directory of repository root"),
            };
            let mut stash_dir = toplevel.clone();
            stash_dir.set_file_name(stash_name);
            let untracked = git.untracked_files()?;
            if !untracked.is_empty() {
                log::info!("Moving untracked files to {} ...", stash_dir.display());
                for path in untracked {
                    let src = toplevel.join(&path);
                    let dest = stash_dir.join(&path);
                    if let Some(p) = dest.parent() {
                        create_dir_all(p)
                            .with_context(|| format!("Failed to create directory {p:?}"))?;
                    }
                    log::debug!("Moving {src:?} to {dest:?}");
                    rename_exclusive(&src, &dest)
                        .with_context(|| format!("Failed to move {src:?} to {dest:?}"))?;
                }
            }

            log::info!("Publishing ...");
            LoggedCommand::new("cargo")
                .arg("publish")
                .arg("--manifest-path")
                .arg(project.manifest_path())
                .status()?;

            if stash_dir.exists() {
                log::info!(
                    "Moving untracked files back from {} ...",
                    stash_dir.display()
                );
                move_dirtree_into(&stash_dir, &toplevel)?;
            }
        }

        log::info!("Pushing tag to GitHub ...");
        git.command().arg("push").arg("--follow-tags").status()?;

        // TODO: Skip this step if using cargo-dist/a `release.yml`
        // workflow:
        log::info!("Creating GitHub release ...");
        let text = git
            .command()
            .arg("show")
            .arg("-s")
            .arg("--format=%s%x00%b")
            .arg(format!("{tag_name}^{{commit}}"))
            .check_output()?;
        let (subject, body) = text.split_once('\0').ok_or_else(|| {
            anyhow::anyhow!("`git show` was asked to output a NUL, but it didn't!")
        })?;
        let release_details = CreateRelease::new(tag_name)
            .name(subject)
            .body(body.trim())
            .prerelease(!new_version.pre.is_empty());
        github.create_release(&ghrepo, release_details)?;

        if activated {
            let mut topics = github
                .get_topics(&ghrepo)?
                .into_iter()
                .collect::<HashSet<_>>();
            let mut changed = false;
            if topics.remove(&Topic::new("work-in-progress")) {
                changed = true;
            }
            if publish && topics.insert(Topic::new("available-on-crates-io")) {
                changed = true;
            }
            if changed {
                log::info!("Updating GitHub repository topics ...");
                github.set_topics(&ghrepo, topics)?;
            }
        }

        log::info!("Preparing for work on next version ...");
        let next_version = bump_version(new_version.clone(), Bump::Minor);

        // Ensure CHANGELOG is present and contains section for upcoming
        // version
        log::info!("Adding next section to CHANGELOG.md ...");
        let mut chlog = project.changelog()?.unwrap_or_else(|| Changelog {
            sections: vec![ChangelogSection {
                header: ChangelogHeader::Released {
                    version: new_version.clone(),
                    date: release_date,
                },
                content: "Initial release".into(),
            }],
        });
        chlog.sections.insert(
            0,
            ChangelogSection {
                header: ChangelogHeader::InProgress {
                    version: next_version.clone(),
                },
                content: String::new(),
            },
        );
        project.set_changelog(chlog)?;

        // Update version in Cargo.toml
        log::info!("Setting next version in Cargo.toml ...");
        project.set_cargo_version(next_version)?;

        // Ensure "Changelog" link is in README
        let Some(mut readme) = project.readme()? else {
            bail!("README.md suddenly disappeared!");
        };
        if readme.ensure_changelog_link(&ghrepo) {
            log::info!("Adding Changelog link to README.md ...");
            project.set_readme(readme)?;
        }

        Ok(())
    }

    fn bump(&self) -> Option<Bump> {
        if self.bump.major {
            Some(Bump::Major)
        } else if self.bump.minor {
            Some(Bump::Minor)
        } else if self.bump.patch {
            Some(Bump::Patch)
        } else {
            None
        }
    }
}

fn parse_v_version(value: &str) -> Result<Version, semver::Error> {
    let value = value.strip_prefix('v').unwrap_or(value);
    value.parse::<Version>()
}

fn write_commit_template<W: Write>(
    mut fp: W,
    version: &Version,
    notes: Option<String>,
) -> io::Result<()> {
    writeln!(fp, "DELETE THIS LINE")?;
    writeln!(fp)?;
    if let Some(notes) = notes {
        writeln!(fp, "v{version} — INSERT SHORT DESCRIPTION HERE")?;
        writeln!(fp)?;
        writeln!(fp, "{}", notes)?;
    } else {
        writeln!(fp, "v{version} — Initial release")?;
    }
    writeln!(fp)?;
    writeln!(fp, "# Write in Markdown.")?;
    writeln!(fp, "# The first line will be used as the release name.")?;
    writeln!(fp, "# The rest will be used as the release body.")?;
    fp.flush()?;
    Ok(())
}
