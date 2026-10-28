//! Read-only facts from the local Postgres database through `psql`.
//!
//! Only SELECT statements on Odoo metadata tables. Names passed in are
//! validated so they cannot change the query.

use std::path::Path;

use serde::Serialize;

use crate::proc;

fn safe_ident(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 120
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
}

async fn query(db: &str, sql: &str) -> Result<Vec<Vec<String>>, String> {
    if !safe_ident(db) {
        return Err(format!("\"{db}\" is not a valid database name"));
    }
    let out = proc::run(
        "psql",
        &[
            "-X",
            "-A",
            "-t",
            "-F",
            "\t",
            "-v",
            "ON_ERROR_STOP=1",
            "-d",
            db,
            "-c",
            sql,
        ],
        Path::new("."),
        proc::SHORT,
    )
    .await?;
    if !out.ok {
        return Err(format!("psql on {db} failed: {}", out.tail(4)));
    }
    Ok(out
        .stdout
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.split('\t').map(str::to_string).collect())
        .collect())
}

/// Databases the current user can connect to.
pub async fn list_databases() -> Result<Vec<String>, String> {
    let rows = query(
        "postgres",
        "SELECT datname FROM pg_database WHERE NOT datistemplate AND datallowconn ORDER BY datname",
    )
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|r| r.into_iter().next())
        .filter(|d| d != "postgres")
        .collect())
}

pub async fn installed_modules(db: &str) -> Result<Vec<String>, String> {
    let rows = query(
        db,
        "SELECT name FROM ir_module_module WHERE state = 'installed' ORDER BY name",
    )
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|r| r.into_iter().next())
        .collect())
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Field {
    pub name: String,
    pub ttype: String,
    pub relation: String,
    pub required: bool,
}

/// Exact field names and types of a model, so the AI does not invent fields.
pub async fn model_fields(db: &str, model: &str) -> Result<Vec<Field>, String> {
    if !safe_ident(model) {
        return Err(format!("\"{model}\" is not a valid model name"));
    }
    let sql = format!(
        "SELECT name, ttype, COALESCE(relation, ''), required FROM ir_model_fields WHERE model = '{model}' ORDER BY name"
    );
    let rows = query(db, &sql).await?;
    Ok(rows
        .into_iter()
        .filter(|r| r.len() >= 4)
        .map(|r| Field {
            name: r[0].clone(),
            ttype: r[1].clone(),
            relation: r[2].clone(),
            required: r[3] == "t",
        })
        .collect())
}

/// Models whose technical name starts with one of the module's model prefixes.
pub async fn models_like(db: &str, needle: &str) -> Result<Vec<String>, String> {
    if !safe_ident(needle) {
        return Ok(Vec::new());
    }
    let sql =
        format!("SELECT model FROM ir_model WHERE model LIKE '%{needle}%' ORDER BY model LIMIT 40");
    let rows = query(db, &sql).await?;
    Ok(rows
        .into_iter()
        .filter_map(|r| r.into_iter().next())
        .collect())
}

/// Compact text block for a context pack.
pub fn format_fields(model: &str, fields: &[Field]) -> String {
    let mut s = format!("{model}:\n");
    for f in fields {
        let rel = if f.relation.is_empty() {
            String::new()
        } else {
            format!(" -> {}", f.relation)
        };
        let req = if f.required { " required" } else { "" };
        s.push_str(&format!("  {} ({}{rel}){req}\n", f.name, f.ttype));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_are_checked() {
        assert!(safe_ident("sale.order.line"));
        assert!(safe_ident("acme_dev"));
        assert!(!safe_ident("x'; drop table y; --"));
        assert!(!safe_ident(""));
    }
}
