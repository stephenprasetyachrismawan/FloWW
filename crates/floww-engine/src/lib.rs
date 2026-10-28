//! FloWW engine: everything that runs work. Used by the desktop app and by
//! the local web server.

pub mod brain;
pub mod db;
pub mod engine;
pub mod events;
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

/// Render PRD markdown as HTML for the viewer and "Copy HTML".
pub fn markdown_to_html(md: &str) -> String {
    let mut opts = pulldown_cmark::Options::empty();
    opts.insert(pulldown_cmark::Options::ENABLE_TABLES);
    opts.insert(pulldown_cmark::Options::ENABLE_STRIKETHROUGH);
    opts.insert(pulldown_cmark::Options::ENABLE_TASKLISTS);
    // Raw HTML in markdown is shown as text, never rendered.
    let events = pulldown_cmark::Parser::new_ext(md, opts).map(|e| match e {
        pulldown_cmark::Event::Html(h) | pulldown_cmark::Event::InlineHtml(h) => {
            pulldown_cmark::Event::Text(h)
        }
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
}
