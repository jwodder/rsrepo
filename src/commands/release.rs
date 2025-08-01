use crate::changelog::{Changelog, ChangelogHeader, ChangelogSection};
use crate::cmd::LoggedCommand;
use crate::github::{CreateRelease, Topic};
use crate::project::{HasReadme, Package, PackageSet, Project};
use crate::provider::Provider;
use crate::readme::{Badge, Repostatus};
use crate::util::{bump_version, move_dirtree_into, this_year, workspace_tag_prefix, Bump};
use anyhow::{bail, Context};
use cargo_metadata::semver::{Op, Prerelease, Version, VersionReq};
use clap::Args;
use ghrepo::LocalRepo;
use renamore::rename_exclusive;
use std::borrow::Cow;
use std::collections::HashSet;
use std::io::{self, Write};
use tempfile::NamedTempFile;

/// Prepare & publish a new release for a package
#[derive(Args, Clone, Debug, Eq, PartialEq)]
pub(crate) struct Release {
    #[command(flatten)]
    pub(crate) bumping: Bumping,

    /// Release the package with the given name in the workspace.
    ///
    /// By default, the package for the current directory is released.
    #[arg(short, long, value_name = "NAME")]
    package: Option<String>,

    /// The version to release.  If neither this argument nor a bump option is
    /// specified, the Cargo.toml version is used without a prerelease or
    /// metadata.
    #[arg(value_parser = parse_v_version, conflicts_with = "bump")]
    version: Option<Version>,
}

impl Release {
    pub(crate) fn run(self, provider: Provider) -> anyhow::Result<()> {
        let github = provider.github()?;
        let project = Project::locate()?;
        let is_workspace = project.project_type().is_workspace();
        let pkgset = project.package_set()?;
        let package = pkgset.get(self.package.as_deref())?;
        let name = package.name();
        let git = project.git();
        let readme_file = package.readme();
        let chlog_file = package.changelog();
        let metadata = package.metadata();
        let old_version = &metadata.version;
        let ghrepo = LocalRepo::new(package.path())
            .github_remote("origin")
            .context("Could not determine GitHub repository for local repository")?;
        let is_lib = package.is_lib();
        let publish = metadata.publish.as_deref() != Some(&[]);
        let Some(default_branch) = git.default_branch()? else {
            bail!("Could not determine repository's default branch");
        };

        let tag_prefix = is_workspace.then(|| workspace_tag_prefix(name));
        // Determine new version
        let new_version = if let Some(v) = self.version {
            v // Skips the checks from the other branch
        } else {
            self.bumping
                .bump(git.latest_tag_version(tag_prefix.as_deref())?, old_version)?
        };
        let tag_prefix = tag_prefix.map_or_else(|| Cow::from(""), Cow::from);
        for v in ["", "v"] {
            let tagname = format!("{tag_prefix}{v}{new_version}");
            if git.tag_exists(&tagname)? {
                bail!("New version already tagged: {tagname}");
            }
        }

        log::info!("Preparing version {new_version} ...");

        let update_lock = project.path().join("Cargo.lock").exists();
        if &new_version != old_version {
            log::info!("Setting version in Cargo.toml ...");
            package.set_cargo_version(&new_version)?;
            bump_dependents(&pkgset, package, &new_version)?;
            if update_lock {
                // Do this AFTER updating dependents!
                package.update_lockfile(&new_version)?;
            }
        }

        let release_date = chrono::Local::now().date_naive();
        let chlog_content;
        if let Some(mut chlog) = chlog_file.get()? {
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
            chlog_file.set(chlog)?;
        } else {
            chlog_content = None;
        };

        let Some(mut readme) = readme_file.get()? else {
            bail!("Package lacks README.md");
        };
        let mut changed = false;
        let activated = if new_version.pre.is_empty()
            && readme.repostatus() == Some(Repostatus::Wip)
        {
            log::info!("Setting repostatus in README.md to Active ...");
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
        if publish && readme.ensure_crates_links(name, is_lib) {
            log::info!("Adding crates.io links to README.md ...");
            changed = true;
        }
        if changed {
            readme_file.set(readme)?;
        }

        log::info!("Updating copyright years in LICENSE ...");
        let mut years = git.commit_years()?;
        years.insert(this_year());
        package.update_license_years(years)?;

        log::info!("Committing ...");
        {
            let mut template = NamedTempFile::new().context("could not create temporary file")?;
            write_commit_template(
                template.as_file_mut(),
                is_workspace.then_some(name),
                &new_version,
                chlog_content,
            )
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
        let tag_name = format!("{tag_prefix}v{new_version}");
        git.command()
            .arg("tag")
            .arg("-s")
            .arg("-m")
            .arg(if is_workspace {
                format!("{name} version {new_version}")
            } else {
                format!("Version {new_version}")
            })
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
                        fs_err::create_dir_all(p)?;
                    }
                    log::debug!("Moving {src:?} to {dest:?}");
                    rename_exclusive(&src, &dest)
                        .with_context(|| format!("Failed to move {src:?} to {dest:?}"))?;
                }
            }

            log::info!("Publishing ...");
            let mut r = LoggedCommand::new("cargo")
                .arg("publish")
                .arg("--manifest-path")
                .arg(package.manifest_path())
                .status()
                .map_err(anyhow::Error::from);

            if stash_dir.exists() {
                log::info!(
                    "Moving untracked files back from {} ...",
                    stash_dir.display()
                );
                let r2 = move_dirtree_into(&stash_dir, &toplevel).map_err(anyhow::Error::from);
                if r.is_err() {
                    if let Err(e) = r2 {
                        log::warn!("{e:?}");
                    }
                } else {
                    r = r2;
                }
            }

            r?;
        }

        log::info!("Pushing tag to GitHub ...");
        git.command().arg("push").arg("--follow-tags").status()?;

        if package
            .path()
            .join(".github")
            .join("workflows")
            .join("release.yml")
            .exists()
        {
            log::info!("release.yml workflow exists; expecting it to create GitHub release");
        } else {
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
        }

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
        let mut dev_next = next_version.clone();
        dev_next.pre =
            Prerelease::new("dev").expect("'dev' should be a valid prerelease identifier");

        // Update version in Cargo.toml
        log::info!("Setting next version in Cargo.toml ...");
        package.set_cargo_version(&dev_next)?;
        bump_dependents(&pkgset, package, &dev_next)?;
        if update_lock {
            // Do this AFTER updating dependents!
            package.update_lockfile(&dev_next)?;
        }

        // Ensure CHANGELOG is present and contains section for upcoming
        // version
        log::info!("Adding next section to CHANGELOG.md ...");
        let mut chlog = chlog_file.get()?.unwrap_or_else(|| Changelog {
            sections: vec![ChangelogSection {
                header: ChangelogHeader::Released {
                    version: new_version,
                    date: release_date,
                },
                content: "Initial release\n".into(),
            }],
        });
        chlog.sections.insert(
            0,
            ChangelogSection {
                header: ChangelogHeader::InProgress {
                    version: next_version,
                },
                content: String::new(),
            },
        );
        chlog_file.set(chlog)?;

        // Ensure "Changelog" link is in README
        let Some(mut readme) = readme_file.get()? else {
            bail!("README.md suddenly disappeared!");
        };
        if readme.ensure_changelog_link(&ghrepo, default_branch) {
            log::info!("Adding Changelog link to README.md ...");
            readme_file.set(readme)?;
        }

        Ok(())
    }
}

#[derive(Args, Clone, Debug, Default, Eq, PartialEq)]
#[group(multiple = false, id = "bump")]
pub(crate) struct Bumping {
    /// Release the next major version
    #[arg(long)]
    major: bool,

    /// Release the next minor version
    #[arg(long)]
    minor: bool,

    /// Release the next patch version
    #[arg(long)]
    patch: bool,
}

impl Bumping {
    fn bump(
        &self,
        tag_version: Option<Version>,
        manifest_version: &Version,
    ) -> anyhow::Result<Version> {
        if let Some(level) = self.level() {
            if let Some(tag_version) = tag_version {
                if !tag_version.pre.is_empty() {
                    bail!("Latest Git tag is a prerelease; cannot bump");
                }
                Ok(bump_version(tag_version, level))
            } else {
                bail!("No Git tag to bump");
            }
        } else {
            if tag_version.is_some_and(|v| v >= *manifest_version) {
                bail!("Latest Git-tagged version exceeds manifest version");
            }
            // Strip any pre-release segment
            Ok(Version::new(
                manifest_version.major,
                manifest_version.minor,
                manifest_version.patch,
            ))
        }
    }

    pub(crate) fn level(&self) -> Option<Bump> {
        if self.major {
            Some(Bump::Major)
        } else if self.minor {
            Some(Bump::Minor)
        } else if self.patch {
            Some(Bump::Patch)
        } else {
            None
        }
    }
}

fn parse_v_version(value: &str) -> Result<Version, cargo_metadata::semver::Error> {
    let value = value.strip_prefix('v').unwrap_or(value);
    value.parse::<Version>()
}

fn bump_dependents(
    pkgset: &PackageSet,
    package: &Package,
    version: &Version,
) -> anyhow::Result<()> {
    let name = package.name();
    for (rname, req) in package.dependents() {
        // When a package `foo`'s version is bumped from `0.3.0-dev` to
        // `0.3.0`, any package `bar` that depends on `foo 0.3.0-dev` should
        // have its version requirement bumped to `0.3.0`, but Cargo's semver
        // rules mean that `^0.3.0-dev` accepts `0.3.0`.  Thus, if `req` using
        // a prelease does not equal `version` being a prerelease, bump.
        if !req.matches(version) || uses_prerelease(req) == version.pre.is_empty() {
            let Some(rpkg) = pkgset.package_by_name(rname) else {
                bail!("Inconsistent project metadata: {name} is depended on by {rname}, but the latter was not found");
            };
            log::info!("Updating {rname}'s dependency on {name} ...");
            rpkg.set_dependency_version(name, version.to_string(), false)?;
        }
    }
    Ok(())
}

fn uses_prerelease(req: &VersionReq) -> bool {
    req.comparators
        .iter()
        .any(|c| c.op == Op::Caret && !c.pre.is_empty())
}

fn write_commit_template<W: Write>(
    mut fp: W,
    package_name: Option<&str>,
    version: &Version,
    notes: Option<String>,
) -> io::Result<()> {
    writeln!(fp, "DELETE THIS LINE")?;
    writeln!(fp)?;
    if let Some(notes) = notes {
        if let Some(name) = package_name {
            writeln!(fp, "{name} v{version} — INSERT SHORT DESCRIPTION HERE")?;
        } else {
            writeln!(fp, "v{version} — INSERT SHORT DESCRIPTION HERE")?;
        }
        writeln!(fp)?;
        writeln!(fp, "{notes}")?;
    } else if let Some(name) = package_name {
        writeln!(fp, "{name} v{version} — Initial release")?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("1.2.3", "1.2.4", "1.2.4")]
    #[case("1.2.3", "1.2.4-dev", "1.2.4")]
    #[case("1.2.3-alpha", "1.2.3-alpha.1", "1.2.3")]
    fn bumping_default(
        #[case] tag_version: Version,
        #[case] manifest_version: Version,
        #[case] bumped: Version,
    ) {
        assert_eq!(
            Bumping::default()
                .bump(Some(tag_version), &manifest_version)
                .unwrap(),
            bumped
        );
    }

    #[rstest]
    #[case("1.2.4", "1.2.4")]
    #[case("1.2.4-dev", "1.2.4")]
    fn bumping_default_no_tag(#[case] manifest_version: Version, #[case] bumped: Version) {
        assert_eq!(
            Bumping::default().bump(None, &manifest_version).unwrap(),
            bumped
        );
    }

    #[rstest]
    #[case("1.2.3", "1.2.3")]
    #[case("1.2.3", "1.2.0")]
    #[case("1.2.3", "1.2.3-dev")]
    #[case("1.2.3", "1.2.2-dev")]
    #[case("1.2.3-alpha.1", "1.2.3-alpha")]
    fn bumping_default_err(#[case] tag_version: Version, #[case] manifest_version: Version) {
        assert!(Bumping::default()
            .bump(Some(tag_version), &manifest_version)
            .is_err());
    }

    #[rstest]
    #[case("1.2.3", "1.2.3", "1.3.0")]
    #[case("1.2.3", "1.2.3-dev", "1.3.0")]
    #[case("1.2.3", "1.3.0-dev", "1.3.0")]
    #[case("1.1.5", "1.2.3", "1.2.0")]
    #[case("1.2.3", "1.1.5", "1.3.0")]
    fn bumping_minor(
        #[case] tag_version: Version,
        #[case] manifest_version: Version,
        #[case] bumped: Version,
    ) {
        let bumping = Bumping {
            minor: true,
            ..Bumping::default()
        };
        assert_eq!(
            bumping.bump(Some(tag_version), &manifest_version).unwrap(),
            bumped,
        );
    }

    #[test]
    fn bumping_minor_pre_tag() {
        let bumping = Bumping {
            minor: true,
            ..Bumping::default()
        };
        let tag_version = "1.2.3-dev".parse::<Version>().unwrap();
        let manifest_version = Version::new(1, 2, 3);
        assert!(bumping.bump(Some(tag_version), &manifest_version).is_err());
    }

    #[test]
    fn bumping_minor_no_tag() {
        let bumping = Bumping {
            minor: true,
            ..Bumping::default()
        };
        let manifest_version = Version::new(1, 2, 3);
        assert!(bumping.bump(None, &manifest_version).is_err());
    }
}
