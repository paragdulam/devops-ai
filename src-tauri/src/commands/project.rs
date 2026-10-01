use crate::project_kind::{self, ProjectKind};
use serde::Serialize;
use std::path::Path;
use walkdir::WalkDir;

/// Directories skipped when counting files/size for a project folder.
/// Reused unchanged by the Milestone 6 archiver, which needs the same exclude list
/// when deciding what to tar for upload to the remote machine.
pub const IGNORE_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    ".venv",
    "__pycache__",
    ".next",
    ".turbo",
];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfo {
    pub name: String,
    pub path: String,
    pub exists: bool,
    pub readable: bool,
    pub is_empty: bool,
    pub file_count: u64,
    pub total_size_bytes: u64,
    pub kind: ProjectKind,
}

fn should_skip(entry: &walkdir::DirEntry) -> bool {
    entry.file_type().is_dir()
        && entry
            .file_name()
            .to_str()
            .map(|name| IGNORE_DIRS.contains(&name))
            .unwrap_or(false)
}

fn walk_stats(root: &Path) -> (u64, u64) {
    let mut file_count = 0u64;
    let mut total_size_bytes = 0u64;

    let walker = WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| !should_skip(e));

    for entry in walker.filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            file_count += 1;
            if let Ok(metadata) = entry.metadata() {
                total_size_bytes += metadata.len();
            }
        }
    }

    (file_count, total_size_bytes)
}

fn build_project_info(path: &str) -> ProjectInfo {
    let p = Path::new(path);
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string());

    if !p.exists() {
        return ProjectInfo {
            name,
            path: path.to_string(),
            exists: false,
            readable: false,
            is_empty: true,
            file_count: 0,
            total_size_bytes: 0,
            kind: ProjectKind::General,
        };
    }

    let dir_entries = std::fs::read_dir(p);
    let readable = dir_entries.is_ok();
    let is_empty = dir_entries
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(true);

    let (file_count, total_size_bytes) = if readable { walk_stats(p) } else { (0, 0) };

    ProjectInfo {
        name,
        path: path.to_string(),
        exists: true,
        readable,
        is_empty,
        file_count,
        total_size_bytes,
        kind: project_kind::classify_local(p),
    }
}

#[tauri::command]
pub async fn inspect_project_folder(path: String) -> Result<ProjectInfo, String> {
    tauri::async_runtime::spawn_blocking(move || build_project_info(&path))
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("remote-dev-machine-test-{}", name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn nonexistent_path_is_flagged() {
        let info = build_project_info("/definitely/does/not/exist/anywhere");
        assert!(!info.exists);
        assert!(!info.readable);
        assert!(info.is_empty);
        assert_eq!(info.file_count, 0);
    }

    #[test]
    fn empty_directory_is_flagged_empty() {
        let dir = temp_dir("empty");
        let info = build_project_info(dir.to_str().unwrap());
        assert!(info.exists);
        assert!(info.readable);
        assert!(info.is_empty);
        assert_eq!(info.file_count, 0);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn counts_files_and_excludes_ignored_dirs() {
        let dir = temp_dir("normal");
        fs::write(dir.join("a.txt"), b"hello").unwrap();
        fs::write(dir.join("b.txt"), b"world!").unwrap();

        let nested = dir.join("src");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("c.txt"), b"nested").unwrap();

        let ignored = dir.join("node_modules");
        fs::create_dir_all(&ignored).unwrap();
        fs::write(ignored.join("dep.js"), b"should not be counted").unwrap();

        let info = build_project_info(dir.to_str().unwrap());
        assert!(info.exists);
        assert!(info.readable);
        assert!(!info.is_empty);
        assert_eq!(info.file_count, 3);
        assert_eq!(info.total_size_bytes, 5 + 6 + 6);

        fs::remove_dir_all(&dir).unwrap();
    }
}
