use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::Error;

fn git(repo: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .args([
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.untrackedCache=false",
            "--no-optional-locks",
        ])
        .current_dir(repo)
        .stdin(Stdio::null());
    for name in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_COMMON_DIR",
        "GIT_CONFIG_PARAMETERS",
        "GIT_EXTERNAL_DIFF",
    ] {
        command.env_remove(name);
    }
    command
}

pub struct RepoPaths {
    pub toplevel: PathBuf,
    pub common_dir: PathBuf,
}

pub fn repo_paths(repo: &Path) -> Result<RepoPaths, Error> {
    let output = git(repo)
        .args(["rev-parse", "--show-toplevel", "--git-common-dir"])
        .output()
        .map_err(|error| Error::Runtime(format!("could not run git: {error}")))?;
    let not_a_repo = || {
        Error::NotARepo(format!(
            "not a git work tree: {}",
            repo.display().to_string().replace(['\r', '\n'], " ")
        ))
    };
    if !output.status.success() {
        return Err(not_a_repo());
    }
    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| Error::Runtime(format!("invalid git path output: {error}")))?;
    let mut lines = stdout.lines();
    let toplevel = PathBuf::from(
        lines
            .next()
            .filter(|line| !line.is_empty())
            .ok_or_else(not_a_repo)?,
    );
    let common_dir = PathBuf::from(
        lines
            .next()
            .filter(|line| !line.is_empty())
            .ok_or_else(not_a_repo)?,
    );
    let common_dir = if common_dir.is_absolute() {
        common_dir
    } else {
        repo.join(common_dir)
    };
    Ok(RepoPaths {
        toplevel,
        common_dir,
    })
}

pub fn ls_files(toplevel: &Path) -> Result<Vec<Vec<u8>>, Error> {
    let output = git(toplevel)
        .args(["ls-files", "-z"])
        .output()
        .map_err(|e| Error::Runtime(format!("could not run git: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let first = stderr.lines().next().unwrap_or("git ls-files failed");
        return Err(Error::Runtime(first.to_owned()));
    }
    Ok(output
        .stdout
        .split(|b| *b == 0)
        .filter(|path| !path.is_empty())
        .map(|path| path.to_vec())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_removes_repo_override_environment() {
        let command = git(Path::new("."));
        let envs: Vec<_> = command.get_envs().collect();
        for name in [
            "GIT_DIR",
            "GIT_WORK_TREE",
            "GIT_INDEX_FILE",
            "GIT_OBJECT_DIRECTORY",
            "GIT_COMMON_DIR",
            "GIT_CONFIG_PARAMETERS",
            "GIT_EXTERNAL_DIFF",
        ] {
            assert!(envs.contains(&(std::ffi::OsStr::new(name), None)));
        }
    }
}
