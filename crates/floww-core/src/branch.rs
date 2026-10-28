//! Development branch names from a template like `dev/{ddmmyyyy}_{slug}`.

/// Default name for new development branches.
pub const DEFAULT_TEMPLATE: &str = "dev/{ddmmyyyy}_{slug}";

/// Tokens a template may use, with what each one becomes.
pub const TOKENS: &[(&str, &str)] = &[
    (
        "{slug}",
        "first words of the request title, e.g. invoice-dobel-pengiriman",
    ),
    ("{type}", "feature, improvement or bugfix"),
    ("{prd_id}", "PRD id, e.g. SP-012"),
    ("{ddmmyyyy}", "date the request was made, e.g. 28102026"),
    ("{yyyymmdd}", "same date, e.g. 20261028"),
    ("{dd}", "day"),
    ("{mm}", "month"),
    ("{yyyy}", "year"),
];

/// What a branch name is built from.
#[derive(Debug, Clone)]
pub struct BranchInput<'a> {
    pub prd_id: &'a str,
    pub slug: &'a str,
    /// `feature`, `improvement` or `bugfix`.
    pub kind: &'a str,
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

/// Check a template. The error says what is wrong and how to fix it.
pub fn validate(template: &str) -> Result<(), String> {
    let t = template.trim();
    if t.is_empty() {
        return Err(format!(
            "Branch name is empty. For example: {DEFAULT_TEMPLATE}"
        ));
    }
    let mut rest = t;
    while let Some(start) = rest.find('{') {
        let after = &rest[start..];
        let Some(end) = after.find('}') else {
            return Err("A { has no closing }. Tokens look like {slug}.".into());
        };
        let token = &after[..=end];
        if !TOKENS.iter().any(|(k, _)| *k == token) {
            let known: Vec<&str> = TOKENS.iter().map(|(k, _)| *k).collect();
            return Err(format!(
                "{token} is not a known token. Use one of: {}.",
                known.join(" ")
            ));
        }
        rest = &after[end + 1..];
    }
    if !t.contains("{slug}") && !t.contains("{prd_id}") {
        return Err(
            "Add {slug} or {prd_id}, otherwise two requests can get the same branch name.".into(),
        );
    }
    let sample = render(
        t,
        &BranchInput {
            prd_id: "SP-001",
            slug: "sample",
            kind: "feature",
            year: 2026,
            month: 1,
            day: 2,
        },
    );
    if sample.is_empty() {
        return Err("This template gives an empty branch name.".into());
    }
    Ok(())
}

/// Fill in the template and make the result a valid git branch name.
pub fn render(template: &str, i: &BranchInput) -> String {
    let raw = template
        .trim()
        .replace("{slug}", i.slug)
        .replace("{type}", i.kind)
        .replace("{prd_id}", i.prd_id)
        .replace(
            "{ddmmyyyy}",
            &format!("{:02}{:02}{:04}", i.day, i.month, i.year),
        )
        .replace(
            "{yyyymmdd}",
            &format!("{:04}{:02}{:02}", i.year, i.month, i.day),
        )
        .replace("{dd}", &format!("{:02}", i.day))
        .replace("{mm}", &format!("{:02}", i.month))
        .replace("{yyyy}", &format!("{:04}", i.year));
    sanitize(&raw)
}

/// Turn any text into something `git check-ref-format --branch` accepts.
pub fn sanitize(name: &str) -> String {
    let mapped: String = name
        .chars()
        .map(|c| match c {
            ' ' | '~' | '^' | ':' | '?' | '*' | '[' | ']' | '\\' | '@' | '{' | '}' => '-',
            c if c.is_control() => '-',
            c => c,
        })
        .collect();
    let parts: Vec<String> = mapped
        .split('/')
        .map(|p| {
            let mut p = p.to_string();
            while p.contains("..") {
                p = p.replace("..", ".");
            }
            while p.contains("--") {
                p = p.replace("--", "-");
            }
            let mut p = p.trim_matches(|c| c == '.' || c == '-').to_string();
            while p.ends_with(".lock") {
                p.truncate(p.len() - ".lock".len());
            }
            p
        })
        .filter(|p| !p.is_empty())
        .collect();
    parts.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> BranchInput<'static> {
        BranchInput {
            prd_id: "SP-012",
            slug: "invoice-dobel-pengiriman",
            kind: "improvement",
            year: 2026,
            month: 10,
            day: 8,
        }
    }

    #[test]
    fn default_template_gives_dev_date_slug() {
        assert_eq!(
            render(DEFAULT_TEMPLATE, &input()),
            "dev/08102026_invoice-dobel-pengiriman"
        );
    }

    #[test]
    fn other_tokens() {
        assert_eq!(
            render("{type}/{yyyymmdd}-{prd_id}", &input()),
            "improvement/20261008-SP-012"
        );
        assert_eq!(
            render("feature/{prd_id}-{slug}", &input()),
            "feature/SP-012-invoice-dobel-pengiriman"
        );
        assert_eq!(
            render("dev/{yyyy}/{mm}/{dd}/{slug}", &input()),
            "dev/2026/10/08/invoice-dobel-pengiriman"
        );
    }

    #[test]
    fn names_are_valid_git_refs() {
        assert_eq!(sanitize("dev//a b..c~d.lock/"), "dev/a-b.c-d");
        assert_eq!(sanitize("-x/.y/z-"), "x/y/z");
        assert_eq!(sanitize("a:b?c*d[e]"), "a-b-c-d-e");
    }

    #[test]
    fn validate_explains_mistakes() {
        assert!(validate(DEFAULT_TEMPLATE).is_ok());
        assert!(
            validate("dev/{date}_{slug}")
                .unwrap_err()
                .contains("not a known token")
        );
        assert!(
            validate("dev/{ddmmyyyy}")
                .unwrap_err()
                .contains("{slug} or {prd_id}")
        );
        assert!(validate("dev/{slug").unwrap_err().contains("no closing"));
        assert!(validate("  ").unwrap_err().contains("empty"));
    }
}
