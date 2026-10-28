//! PRD ids: user initials plus a per-user counter, e.g. `SP-012`.
//!
//! Each user has their own counter file in the brain
//! (`_floww/.counters/<INITIALS>.json`), so two teammates never collide.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

#[derive(Debug, Default, Serialize, Deserialize)]
struct Counter {
    last: u32,
}

/// Initials from a git user name: "Stephen Prasetya" -> "SP", "andi" -> "AN".
pub fn initials_from_name(name: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().collect();
    let letters: String = match words.as_slice() {
        [] => String::new(),
        [one] => one
            .chars()
            .filter(|c| c.is_alphanumeric())
            .take(2)
            .collect(),
        [first, .., last] => [first, last]
            .iter()
            .filter_map(|w| w.chars().find(|c| c.is_alphanumeric()))
            .collect(),
    };
    letters.to_uppercase()
}

pub fn format_id(initials: &str, n: u32) -> String {
    format!("{initials}-{n:03}")
}

fn counter_path(brain: &Path, initials: &str) -> PathBuf {
    brain
        .join("_floww")
        .join(".counters")
        .join(format!("{initials}.json"))
}

fn validate(initials: &str) -> Result<()> {
    let ok = (1..=4).contains(&initials.len())
        && initials
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
    if ok {
        Ok(())
    } else {
        Err(Error::Invalid {
            field: "initials",
            message: format!("\"{initials}\" must be 1–4 capital letters or digits"),
        })
    }
}

/// Reserve the next id for this user and persist the counter.
pub fn next_id(brain: &Path, initials: &str) -> Result<String> {
    validate(initials)?;
    let path = counter_path(brain, initials);
    let mut counter: Counter = match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).map_err(|e| Error::Json {
            path: path.clone(),
            message: e.to_string(),
        })?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Counter::default(),
        Err(source) => return Err(Error::Read { path, source }),
    };
    counter.last += 1;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| Error::Write {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let text = serde_json::to_string(&counter).expect("counter serializes");
    std::fs::write(&path, text).map_err(|source| Error::Write {
        path: path.clone(),
        source,
    })?;
    Ok(format_id(initials, counter.last))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initials() {
        assert_eq!(initials_from_name("Stephen Prasetya Chrismawan"), "SC");
        assert_eq!(initials_from_name("Andi Rahman"), "AR");
        assert_eq!(initials_from_name("andi"), "AN");
        assert_eq!(initials_from_name("  "), "");
    }

    #[test]
    fn ids_count_up_per_user() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(next_id(dir.path(), "SP").unwrap(), "SP-001");
        assert_eq!(next_id(dir.path(), "SP").unwrap(), "SP-002");
        assert_eq!(next_id(dir.path(), "AR").unwrap(), "AR-001");
        assert_eq!(next_id(dir.path(), "SP").unwrap(), "SP-003");
    }

    #[test]
    fn rejects_bad_initials() {
        let dir = tempfile::tempdir().unwrap();
        assert!(next_id(dir.path(), "sp").is_err());
        assert!(next_id(dir.path(), "../x").is_err());
        assert!(next_id(dir.path(), "").is_err());
    }

    #[test]
    fn pads_to_three_digits() {
        assert_eq!(format_id("SP", 7), "SP-007");
        assert_eq!(format_id("SP", 1234), "SP-1234");
    }
}
