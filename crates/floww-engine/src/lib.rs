//! FloWW engine: everything that runs work. Used by the desktop app and by
//! the local web server.

pub mod brain;
pub mod db;
pub mod engine;
pub mod events;
pub mod folders;
pub mod git;
pub mod github;
pub mod jsonx;
pub mod lock;
pub mod model;
pub mod odoo_facts;
pub mod paths;
pub mod proc;
pub mod prompts;
pub mod runner;
pub mod secrets;
pub mod setup;
pub mod skills;
pub mod store;
pub mod worktree_files;

pub use engine::Engine;

use pulldown_cmark::{CowStr, Event, Tag};

/// Links and images may only point at web pages, mail, or anchors.
/// PRD text comes from the AI and could carry `javascript:` links.
fn safe_url(url: &str) -> bool {
    let u = url.trim().to_ascii_lowercase();
    u.starts_with("http://")
        || u.starts_with("https://")
        || u.starts_with("mailto:")
        || u.starts_with('#')
}

/// Render PRD markdown as HTML for the viewer and "Copy HTML".
pub fn markdown_to_html(md: &str) -> String {
    let mut opts = pulldown_cmark::Options::empty();
    opts.insert(pulldown_cmark::Options::ENABLE_TABLES);
    opts.insert(pulldown_cmark::Options::ENABLE_STRIKETHROUGH);
    opts.insert(pulldown_cmark::Options::ENABLE_TASKLISTS);
    let events = pulldown_cmark::Parser::new_ext(md, opts).map(|e| match e {
        // Raw HTML in markdown is shown as text, never rendered.
        Event::Html(h) | Event::InlineHtml(h) => Event::Text(h),
        Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) if !safe_url(&dest_url) => Event::Start(Tag::Link {
            link_type,
            dest_url: CowStr::Borrowed("#"),
            title,
            id,
        }),
        Event::Start(Tag::Image {
            link_type,
            dest_url,
            title,
            id,
        }) if !safe_url(&dest_url) => Event::Start(Tag::Image {
            link_type,
            dest_url: CowStr::Borrowed(""),
            title,
            id,
        }),
        other => other,
    });
    let mut out = String::new();
    pulldown_cmark::html::push_html(&mut out, events);
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn markdown_escapes_raw_html() {
        let html = super::markdown_to_html(
            "# T\n\n<script>alert(1)</script>\n\n| a | b |\n|---|---|\n| 1 | 2 |\n",
        );
        assert!(html.contains("<h1>T</h1>"));
        assert!(!html.contains("<script>"));
        assert!(html.contains("<table>"));
    }

    #[test]
    fn markdown_drops_script_links() {
        let html = super::markdown_to_html(
            "[click](javascript:alert(1)) [ok](https://odoo.com) ![x](data:image/svg+xml,evil)",
        );
        assert!(!html.contains("javascript:"), "{html}");
        assert!(!html.contains("data:image"), "{html}");
        assert!(html.contains("https://odoo.com"));
    }

    #[test]
    fn brain_paths_stay_inside_the_vault() {
        use crate::engine::safe_rel;
        assert!(safe_rel("rules/no-double-invoice.md"));
        assert!(safe_rel(""));
        assert!(!safe_rel("/etc/passwd"));
        assert!(!safe_rel("../secrets"));
        assert!(!safe_rel("rules/../../x"));
        assert!(!safe_rel("a\0b"));
    }
}
