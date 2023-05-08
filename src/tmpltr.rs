use anyhow::{bail, Context as _};
use include_dir::{include_dir, Dir, DirEntry};
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::fs::{create_dir_all, File};
use std::io::BufWriter;
use std::path::Path;
use tera::{Context, Tera, Value};

static TEMPLATE_DATA: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/src/templates");

#[derive(Debug)]
pub struct Templater {
    engine: Tera,
}

impl Templater {
    pub fn load() -> anyhow::Result<Self> {
        let mut engine = Tera::default();
        log::debug!("Loading Tera templates");
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
                            .add_raw_template(path, content)
                            .with_context(|| format!("Failed to load template {path}"))?;
                    }
                }
            }
        }
        engine.register_filter("toml_escape", toml_escape);
        Ok(Templater { engine })
    }

    pub fn render_file<S: Serialize>(
        &self,
        dirpath: &Path,
        template: &str,
        context: S,
    ) -> anyhow::Result<()> {
        let context = Context::from_serialize(context)
            .context("Failed to construct Context from Serialize value")?;
        let path = dirpath.join(template);
        create_dir_all(path.parent().unwrap())
            .with_context(|| format!("Failed to create parent directory for {}", path.display()))?;
        let fp = BufWriter::new(
            File::create(&path)
                .with_context(|| format!("Error creating file {}", path.display()))?,
        );
        self.engine
            .render_to(&format!("{template}.tera"), &context, fp)
            .with_context(|| format!("Failed to render template {template:?} to file"))
    }

    pub fn render_str<S: Serialize>(
        &mut self,
        template_content: &str,
        context: S,
    ) -> anyhow::Result<String> {
        let context = Context::from_serialize(context)
            .context("Failed to construct Context from Serialize value")?;
        self.engine
            .render_str(template_content, &context)
            .context("Failed to render dynamic template")
    }
}

fn toml_escape(value: &Value, _: &HashMap<String, Value>) -> Result<Value, tera::Error> {
    let Value::String(s) = value else {
        return Err(tera::Error::msg("toml_escape can only escape strings"));
    };
    let mut out = String::new();
    for ch in s.chars() {
        match ch {
            '"' | '\\' => {
                out.push('\\');
                out.push(ch);
            }
            '\x08' => out.push_str(r#"\b"#),
            '\t' => out.push_str(r#"\t"#),
            '\n' => out.push_str(r#"\n"#),
            '\x0C' => out.push_str(r#"\f"#),
            '\r' => out.push_str(r#"\r"#),
            '\x00'..='\x1F' | '\x7F' => out.push_str(&format!("\\u{:04}", ch as u32)),
            _ => out.push(ch),
        }
    }
    Ok(Value::String(out))
}
