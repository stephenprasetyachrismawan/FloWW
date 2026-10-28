//! PRD runs on disk: `.floww/runs/<id>.json`, written atomically.

use std::path::Path;

use crate::model::Prd;

pub fn save(runs: &Path, prd: &Prd) -> Result<(), String> {
    std::fs::create_dir_all(runs)
        .map_err(|e| format!("could not create {}: {e}", runs.display()))?;
    let path = runs.join(format!("{}.json", prd.id));
    let tmp = runs.join(format!(".{}.json.tmp", prd.id));
    let text = serde_json::to_string_pretty(prd).map_err(|e| e.to_string())?;
    std::fs::write(&tmp, text).map_err(|e| format!("could not write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("could not save {}: {e}", path.display()))
}

pub fn load_all(runs: &Path) -> Vec<Prd> {
    let Ok(dir) = std::fs::read_dir(runs) else {
        return Vec::new();
    };
    let mut out: Vec<Prd> = dir
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "json").unwrap_or(false))
        .filter(|p| {
            !p.file_name()
                .map(|n| n.to_string_lossy().starts_with('.'))
                .unwrap_or(false)
        })
        .filter_map(|p| {
            let text = std::fs::read_to_string(&p).ok()?;
            match serde_json::from_str::<Prd>(&text) {
                Ok(prd) => Some(prd),
                Err(e) => {
                    tracing::warn!("skipping {}: {e}", p.display());
                    None
                }
            }
        })
        .collect();
    out.sort_by_key(|p| std::cmp::Reverse(p.created_at));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::PrdType;

    #[test]
    fn round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let p = Prd::new(
            "SP-001".into(),
            "Fix it".into(),
            "d".into(),
            PrdType::Bug,
            "Dewi".into(),
            "SP".into(),
        );
        save(dir.path(), &p).unwrap();
        let all = load_all(dir.path());
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, "SP-001");
    }
}
