//! Facts from Odoo source code: modules, models, inheritance, fields, views.
//!
//! A light line-based reader, not a full Python parser. It is deterministic
//! and good enough for anchors and the graph; the AI never writes these facts.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct ModelFact {
    pub name: String,
    pub inherits: Vec<String>,
    pub fields: Vec<String>,
    pub methods: Vec<String>,
    pub file: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct ModuleFact {
    pub name: String,
    pub path: String,
    pub version: String,
    pub depends: Vec<String>,
    pub models: Vec<ModelFact>,
    pub views: Vec<String>,
}

const SKIP_DIRS: &[&str] = &[
    ".git",
    ".floww",
    "node_modules",
    "__pycache__",
    ".venv",
    "venv",
    "target",
];

fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > 6 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            let name = e.file_name().to_string_lossy().into_owned();
            if SKIP_DIRS.contains(&name.as_str()) {
                continue;
            }
            if p.join("__manifest__.py").is_file() {
                out.push(p.clone());
                continue;
            }
            walk(&p, depth + 1, out);
        }
    }
}

/// Folders that contain a `__manifest__.py`.
pub fn module_dirs(repo: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk(repo, 0, &mut out);
    out.sort();
    out
}

fn quoted_strings(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\'' || c == '"' {
            let mut v = String::new();
            for n in chars.by_ref() {
                if n == c {
                    break;
                }
                v.push(n);
            }
            out.push(v);
        }
    }
    out
}

fn manifest_value(text: &str, key: &str) -> Option<String> {
    let pat_a = format!("'{key}'");
    let pat_b = format!("\"{key}\"");
    let idx = text.find(&pat_a).or_else(|| text.find(&pat_b))?;
    let rest = &text[idx + key.len() + 2..];
    let rest = rest.trim_start().strip_prefix(':')?.trim_start();
    if rest.starts_with('[') {
        let end = rest.find(']')?;
        Some(rest[..=end].to_string())
    } else {
        quoted_strings(rest.lines().next()?).into_iter().next()
    }
}

fn read_models(file: &Path, rel: &str) -> Vec<ModelFact> {
    let Ok(text) = std::fs::read_to_string(file) else {
        return Vec::new();
    };
    let mut models = Vec::new();
    let mut cur: Option<ModelFact> = None;
    for line in text.lines() {
        let t = line.trim_start();
        let indent = line.len() - t.len();
        if indent == 0 && t.starts_with("class ") {
            if let Some(m) = cur.take().filter(|m| !m.name.is_empty()) {
                models.push(m);
            }
            let is_model = t.contains("models.Model")
                || t.contains("models.TransientModel")
                || t.contains("models.AbstractModel");
            cur = is_model.then(|| ModelFact {
                file: rel.to_string(),
                ..Default::default()
            });
            continue;
        }
        let Some(m) = cur.as_mut() else { continue };
        if indent == 0 && !t.is_empty() && !t.starts_with('#') && !t.starts_with('@') {
            if let Some(m) = cur.take().filter(|m| !m.name.is_empty()) {
                models.push(m);
            }
            continue;
        }
        if let Some(v) = t.strip_prefix("_name") {
            if let Some(n) = quoted_strings(v).into_iter().next() {
                m.name = n;
            }
        } else if let Some(v) = t.strip_prefix("_inherit") {
            m.inherits = quoted_strings(v);
            if m.name.is_empty()
                && let Some(first) = m.inherits.first()
            {
                m.name = first.clone();
            }
        } else if let Some(def) = t.strip_prefix("def ") {
            if indent > 0
                && let Some(name) = def.split('(').next()
            {
                m.methods.push(name.trim().to_string());
            }
        } else if t.contains("= fields.")
            && indent > 0
            && let Some(name) = t.split('=').next()
        {
            let name = name.trim();
            if name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                m.fields.push(name.to_string());
            }
        }
    }
    if let Some(m) = cur.filter(|m| !m.name.is_empty()) {
        models.push(m);
    }
    models
}

fn files_with_ext(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            let name = e.file_name().to_string_lossy().into_owned();
            if !SKIP_DIRS.contains(&name.as_str()) && name != "tests" {
                files_with_ext(&p, ext, out);
            }
        } else if p.extension().map(|x| x == ext).unwrap_or(false) {
            out.push(p);
        }
    }
}

pub fn read_module(repo: &Path, dir: &Path) -> ModuleFact {
    let manifest = std::fs::read_to_string(dir.join("__manifest__.py")).unwrap_or_default();
    let rel = |p: &Path| {
        p.strip_prefix(repo)
            .unwrap_or(p)
            .to_string_lossy()
            .into_owned()
    };
    let mut py = Vec::new();
    files_with_ext(dir, "py", &mut py);
    py.sort();
    let mut xml = Vec::new();
    files_with_ext(dir, "xml", &mut xml);
    xml.sort();
    ModuleFact {
        name: dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        path: rel(dir),
        version: manifest_value(&manifest, "version").unwrap_or_default(),
        depends: manifest_value(&manifest, "depends")
            .map(|v| quoted_strings(&v))
            .unwrap_or_default(),
        models: py.iter().flat_map(|f| read_models(f, &rel(f))).collect(),
        views: xml.iter().map(|f| rel(f)).collect(),
    }
}

pub fn read_all(repo: &Path) -> Vec<ModuleFact> {
    module_dirs(repo)
        .iter()
        .map(|d| read_module(repo, d))
        .collect()
}

/// Graph for the Brain page: modules, models and their links.
#[derive(Debug, Clone, Serialize, Default)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Node {
    pub id: String,
    pub label: String,
    pub kind: &'static str,
}

pub fn graph(modules: &[ModuleFact]) -> Graph {
    let mut nodes: BTreeMap<String, Node> = BTreeMap::new();
    let mut edges = Vec::new();
    let local: Vec<&str> = modules.iter().map(|m| m.name.as_str()).collect();
    for m in modules {
        let mid = format!("module:{}", m.name);
        nodes.insert(
            mid.clone(),
            Node {
                id: mid.clone(),
                label: m.name.clone(),
                kind: "module",
            },
        );
        for d in &m.depends {
            if local.contains(&d.as_str()) {
                edges.push((mid.clone(), format!("module:{d}")));
            }
        }
        for model in &m.models {
            let id = format!("model:{}", model.name);
            nodes.entry(id.clone()).or_insert(Node {
                id: id.clone(),
                label: model.name.clone(),
                kind: "model",
            });
            edges.push((mid.clone(), id));
        }
    }
    edges.sort();
    edges.dedup();
    Graph {
        nodes: nodes.into_values().collect(),
        edges,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_models_fields_and_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let m = dir.path().join("addons/sale_custom");
        std::fs::create_dir_all(m.join("models")).unwrap();
        std::fs::write(
            m.join("__manifest__.py"),
            "{\n 'name': 'Sale custom',\n 'version': '17.0.1.4.2',\n 'depends': ['sale', 'stock'],\n}",
        )
        .unwrap();
        std::fs::write(
            m.join("models/sale.py"),
            "from odoo import fields, models\n\nclass SaleOrderLine(models.Model):\n    _inherit = 'sale.order.line'\n\n    x_done = fields.Float()\n\n    def _compute_qty_to_invoice(self):\n        pass\n\nclass Tier(models.Model):\n    _name = \"customer.discount.tier\"\n    threshold = fields.Monetary()\n",
        )
        .unwrap();
        let mods = read_all(dir.path());
        assert_eq!(mods.len(), 1);
        let s = &mods[0];
        assert_eq!(s.name, "sale_custom");
        assert_eq!(s.version, "17.0.1.4.2");
        assert_eq!(s.depends, vec!["sale", "stock"]);
        assert_eq!(s.models.len(), 2);
        assert_eq!(s.models[0].name, "sale.order.line");
        assert_eq!(s.models[0].fields, vec!["x_done"]);
        assert_eq!(s.models[0].methods, vec!["_compute_qty_to_invoice"]);
        assert_eq!(s.models[1].name, "customer.discount.tier");
        let g = graph(&mods);
        assert!(
            g.nodes
                .iter()
                .any(|n| n.id == "model:customer.discount.tier")
        );
    }
}
