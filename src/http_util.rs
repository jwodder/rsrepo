#![allow(dead_code)]
use indent_write::indentable::Indentable;
use mime::{Mime, JSON};
use serde_json::{to_string_pretty, value::Value};
use ureq::Response;

/// Error raised for a 4xx or 5xx HTTP response that includes the response body
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatusError {
    url: String,
    method: String,
    status: String,
    body: Option<String>,
}

impl StatusError {
    pub fn for_response(method: &str, r: Response) -> StatusError {
        let url = r.get_url().to_string();
        let status = format!("{} {}", r.status(), r.status_text());
        // If the response body is JSON, pretty-print it.
        let body = if is_json_response(&r) {
            r.into_json::<Value>()
                .ok()
                .map(|v| to_string_pretty(&v).unwrap())
        } else {
            r.into_string().ok()
        };
        StatusError {
            url,
            status,
            body,
            method: method.to_string(),
        }
    }
}

impl std::fmt::Display for StatusError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{} request to {} returned {}",
            self.method, self.url, self.status
        )?;
        if let Some(text) = &self.body {
            write!(f, "\n\n{}\n", text.indented("    "))?;
        }
        Ok(())
    }
}

impl std::error::Error for StatusError {}

/// Return the `rel="next"` URL, if any, from the response's "Link" header
pub fn get_next_link(r: &Response) -> Option<String> {
    Some(
        parse_link_header::parse_with_rel(r.header("Link")?)
            .ok()?
            .get("next")?
            .raw_uri
            .clone(),
    )
}

/// Returns `true` iff the response's Content-Type header indicates the body is
/// JSON
pub fn is_json_response(r: &Response) -> bool {
    r.header("Content-Type")
        .and_then(|v| v.parse::<Mime>().ok())
        .map(|ct| {
            ct.type_() == "application" && (ct.subtype() == "json" || ct.suffix() == Some(JSON))
        })
        .unwrap_or(false)
}
