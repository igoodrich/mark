//! Turns a Markdown file into the HTML page Mark shows.

use pulldown_cmark::{CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd, html};
use std::collections::HashMap;

const STYLE: &str = include_str!("../assets/style.css");
const SCRIPT: &str = include_str!("../assets/keys.js");

/// Renders Markdown source to an HTML fragment, giving every heading a
/// GitHub-style id so in-document `#anchor` links and tables of contents work.
pub fn body(source: &str) -> String {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_HEADING_ATTRIBUTES
        | Options::ENABLE_GFM;
    let mut events: Vec<Event> = Parser::new_ext(source, options).collect();
    add_heading_ids(&mut events);
    let mut out = String::with_capacity(source.len() * 3 / 2);
    html::push_html(&mut out, events.into_iter());
    out
}

/// Wraps a rendered body in the full page: theme colors, base styles, and the
/// keyboard script. The nonce lets only our own script run, so `<script>` or
/// `onerror=` attributes inside a Markdown file stay inert.
pub fn page(title: &str, body: &str, theme_css: &str) -> String {
    let nonce = nonce();
    format!(
        r#"<!doctype html>
<html><head><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src mark: data:; img-src mark: data: https: http:; style-src 'unsafe-inline' mark:; script-src 'nonce-{nonce}'">
<title>{title}</title>
<style id="theme">{theme_css}</style>
<style>{STYLE}</style>
</head><body>
<main id="content">{body}</main>
<script nonce="{nonce}">{SCRIPT}</script>
</body></html>"#,
        title = escape(title),
    )
}

fn add_heading_ids(events: &mut [Event]) {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut i = 0;
    while i < events.len() {
        if let Event::Start(Tag::Heading { id: None, .. }) = &events[i] {
            let mut text = String::new();
            let mut j = i + 1;
            while j < events.len() && !matches!(events[j], Event::End(TagEnd::Heading(_))) {
                if let Event::Text(t) | Event::Code(t) = &events[j] {
                    text.push_str(t);
                }
                j += 1;
            }
            let base = slug(&text);
            let n = seen.entry(base.clone()).or_insert(0);
            let id = if *n == 0 { base } else { format!("{base}-{n}") };
            *n += 1;
            if let Event::Start(Tag::Heading { level, classes, attrs, .. }) = &events[i] {
                let (level, classes, attrs): (HeadingLevel, _, _) = (*level, classes.clone(), attrs.clone());
                events[i] = Event::Start(Tag::Heading { level, id: Some(CowStr::from(id)), classes, attrs });
            }
            i = j;
        }
        i += 1;
    }
}

/// GitHub's anchor rule: lowercase, drop punctuation, spaces become hyphens.
fn slug(text: &str) -> String {
    text.trim()
        .to_lowercase()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            c if c.is_alphanumeric() || c == '-' || c == '_' => Some(c),
            _ => None,
        })
        .collect()
}

pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn nonce() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos());
    format!("{:016x}", h.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_get_unique_github_ids() {
        let html = body("# Hello, World!\n\n## Hello, World!\n\n### `code` bit\n");
        assert!(html.contains(r#"<h1 id="hello-world">"#), "{html}");
        assert!(html.contains(r#"<h2 id="hello-world-1">"#), "{html}");
        assert!(html.contains(r#"<h3 id="code-bit">"#), "{html}");
    }

    #[test]
    fn explicit_heading_ids_are_kept() {
        assert!(body("# Title {#custom}\n").contains(r#"id="custom""#));
    }

    #[test]
    fn gfm_features_render() {
        let html = body("| a | b |\n|---|---|\n| 1 | 2 |\n\n- [x] done\n\n~~gone~~\n");
        assert!(html.contains("<table>"));
        assert!(html.contains("checkbox"));
        assert!(html.contains("<del>gone</del>"));
    }

    #[test]
    fn page_escapes_title_and_sets_nonce_csp() {
        let p = page("<x>.md", "<p>hi</p>", ":root{}");
        assert!(p.contains("<title>&lt;x&gt;.md</title>"));
        assert!(p.contains("script-src 'nonce-"));
    }
}
