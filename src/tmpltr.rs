use anyhow::{bail, Context as _};
use include_dir::{include_dir, Dir, DirEntry};
use serde::Serialize;
use serde_json::Value;
use std::collections::VecDeque;
use std::fs::{create_dir_all, write};
use std::path::Path;
use tinytemplate::{error::Error, format_unescaped, TinyTemplate};

static TEMPLATE_DATA: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/src/templates");

pub struct Templater<'a> {
    engine: TinyTemplate<'a>,
}

impl<'a> Templater<'a> {
    pub fn load() -> anyhow::Result<Self> {
        let mut engine = TinyTemplate::new();
        log::debug!("Loading templates");
        let mut dirs = VecDeque::from([&TEMPLATE_DATA]);
        loop {
            let Some(d) = dirs.pop_front() else { break };
            for entry in d.entries() {
                match entry {
                    DirEntry::Dir(entry) => dirs.push_back(entry),
                    DirEntry::File(file) => {
                        let Some(path) = file.path().to_str() else {
                            bail!("Template path is not UTF-8: {:?}", file.path());
                        };
                        let Some(content) = file.contents_utf8() else {
                            bail!("Template source is not UTF-8: {path}");
                        };
                        engine
                            .add_template(path, content)
                            .with_context(|| format!("Failed to load template {path}"))?;
                    }
                }
            }
        }
        engine.add_formatter("toml_escape", toml_escape);
        engine.set_default_formatter(&format_unescaped);
        Ok(Templater { engine })
    }

    pub fn render_file<S: Serialize>(
        &self,
        dirpath: &Path,
        template: &str,
        context: S,
    ) -> anyhow::Result<()> {
        let path = dirpath.join(template);
        create_dir_all(path.parent().expect("path should have a parent directory"))
            .with_context(|| format!("Failed to create parent directory for {}", path.display()))?;
        let content = self
            .engine
            .render(&format!("{template}.tt"), &context)
            .with_context(|| format!("Failed to render template {template:?}"))?;
        write(&path, content)
            .with_context(|| format!("Failed to write templated text to {}", path.display()))?;
        Ok(())
    }

    pub fn render_str<S: Serialize>(
        &mut self,
        template_content: &'a str,
        context: S,
    ) -> anyhow::Result<String> {
        self.engine
            .add_template("__str", template_content)
            .context("Failed to register dynamic template")?;
        self.engine
            .render("__str", &context)
            .context("Failed to render dynamic template")
    }
}

fn toml_escape(value: &Value, out: &mut String) -> Result<(), Error> {
    let Value::String(s) = value else {
        return Err(Error::GenericError {
            msg: "toml_escape can only escape strings".into(),
        });
    };
    for ch in s.chars() {
        match ch {
            '"' | '\\' => {
                out.push('\\');
                out.push(ch);
            }
            '\x08' => out.push_str(r"\b"),
            '\t' => out.push_str(r"\t"),
            '\n' => out.push_str(r"\n"),
            '\x0C' => out.push_str(r"\f"),
            '\r' => out.push_str(r"\r"),
            '\x00'..='\x1F' | '\x7F' => out.push_str(&format!("\\u{:04}", ch as u32)),
            _ => out.push(ch),
        }
    }
    Ok(())
}
