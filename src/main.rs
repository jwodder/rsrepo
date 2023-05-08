mod cmd;
mod project;
use crate::project::Project;

fn main() -> anyhow::Result<()> {
    let project = Project::locate()?;
    println!("{}", project.path().display());
    Ok(())
}
