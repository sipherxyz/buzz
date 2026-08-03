use std::path::{Path, PathBuf};

fn profile_target_dirs(root: &Path) -> [PathBuf; 2] {
    if cfg!(debug_assertions) {
        // `just dev` builds fresh debug sidecars; never prefer stale release output.
        [root.join("target/debug"), root.join("target/release")]
    } else {
        [root.join("target/release"), root.join("target/debug")]
    }
}

pub(super) fn command_search_dirs_from(
    workspace_root: &Path,
    current_dir: Option<&Path>,
    executable_path: Option<&Path>,
) -> Vec<PathBuf> {
    let mut dirs = executable_path
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .into_iter()
        .collect::<Vec<_>>();
    dirs.extend(profile_target_dirs(workspace_root));
    if let Some(current_dir) = current_dir {
        dirs.extend(profile_target_dirs(current_dir));
    }

    dirs.into_iter().fold(Vec::new(), |mut unique, dir| {
        if !unique.contains(&dir) {
            unique.push(dir);
        }
        unique
    })
}

pub(super) fn command_search_dirs() -> Vec<PathBuf> {
    let current_dir = std::env::current_dir().ok();
    let executable_path = std::env::current_exe().ok();
    command_search_dirs_from(
        &super::workspace_root_dir(),
        current_dir.as_deref(),
        executable_path.as_deref(),
    )
}
