use anyhow::{Context, Result};
use git2::{Repository, Signature, Status, StatusOptions};
use std::path::{Path, PathBuf};

/// Git operations for the autoresearch loop.
pub struct GitRepo {
    repo: Repository,
}

/// Status of the working tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorktreeStatus {
    Clean,
    /// Has changes, but only in autoresearch-owned artifacts.
    OnlyArtifacts,
    /// Has unrelated user changes.
    Dirty(Vec<String>),
}

/// Autoresearch-owned file basenames that are allowed to be dirty.
const OWNED_ARTIFACTS: &[&str] = &[
    "results.tsv",
    "state.json",
    "context.json",
    "lessons.md",
    "launch.json",
    "runtime.json",
    "runtime.log",
];

const OWNED_DIRS: &[&str] = &["autoresearch-results"];

impl GitRepo {
    /// Open a git repository at the given path.
    pub fn open(path: &Path) -> Result<Self> {
        let repo = Repository::discover(path).context("Not a git repository")?;
        Ok(Self { repo })
    }

    /// Get the current HEAD commit hash (short).
    pub fn head_short(&self) -> Result<String> {
        let head = self.repo.head().context("No HEAD")?;
        let oid = head.target().context("HEAD has no target")?;
        Ok(oid.to_string()[..7].to_string())
    }

    /// Get the current HEAD commit hash (full).
    pub fn head_full(&self) -> Result<String> {
        let head = self.repo.head().context("No HEAD")?;
        let oid = head.target().context("HEAD has no target")?;
        Ok(oid.to_string())
    }

    /// Return true when the current HEAD starts with the provided short or full hash.
    pub fn head_matches(&self, commit: &str) -> Result<bool> {
        Ok(self.head_full()?.starts_with(commit))
    }

    /// Get the current HEAD commit summary.
    pub fn head_summary(&self) -> Result<String> {
        let head = self.repo.head()?.peel_to_commit()?;
        Ok(head.summary().unwrap_or("").to_string())
    }

    /// Return true when HEAD is detached instead of pointing at a branch.
    pub fn head_detached(&self) -> Result<bool> {
        self.repo.head_detached().context("Failed to inspect HEAD")
    }

    /// Return known git lock files that can block safe commit/status operations.
    pub fn lock_files(&self) -> Vec<PathBuf> {
        const LOCK_FILES: &[&str] = &["index.lock", "HEAD.lock", "config.lock", "packed-refs.lock"];

        let git_dir = self.repo.path();
        LOCK_FILES
            .iter()
            .map(|name| git_dir.join(name))
            .filter(|path| path.exists())
            .collect()
    }

    /// Return autoresearch-owned artifacts that are staged in the git index.
    pub fn staged_owned_artifacts(&self) -> Result<Vec<String>> {
        let mut opts = StatusOptions::new();
        opts.include_untracked(true);
        let statuses = self.repo.statuses(Some(&mut opts))?;
        let staged_mask = Status::INDEX_NEW
            | Status::INDEX_MODIFIED
            | Status::INDEX_DELETED
            | Status::INDEX_RENAMED
            | Status::INDEX_TYPECHANGE;

        Ok(statuses
            .iter()
            .filter_map(|entry| {
                let path = entry.path()?;
                if entry.status().intersects(staged_mask) && is_owned_artifact(path) {
                    Some(path.to_string())
                } else {
                    None
                }
            })
            .collect())
    }

    /// Check the working tree status.
    pub fn worktree_status(&self) -> Result<WorktreeStatus> {
        let mut opts = StatusOptions::new();
        opts.include_untracked(true);
        let statuses = self.repo.statuses(Some(&mut opts))?;

        if statuses.is_empty() {
            return Ok(WorktreeStatus::Clean);
        }

        let mut unrelated = Vec::new();
        for entry in statuses.iter() {
            let path = entry.path().unwrap_or("");
            if !is_owned_artifact(path) {
                unrelated.push(path.to_string());
            }
        }

        if unrelated.is_empty() {
            Ok(WorktreeStatus::OnlyArtifacts)
        } else {
            Ok(WorktreeStatus::Dirty(unrelated))
        }
    }

    /// Stage files matching the given glob patterns within scope.
    pub fn stage_scoped(&self, paths: &[&Path]) -> Result<()> {
        let mut index = self.repo.index()?;
        for path in paths {
            // Path should be relative to repo root
            index
                .add_path(path)
                .with_context(|| format!("Failed to stage {}", path.display()))?;
        }
        index.write()?;
        Ok(())
    }

    /// Create an experiment commit with the given message.
    pub fn commit_experiment(&self, message: &str) -> Result<String> {
        let mut index = self.repo.index()?;
        let tree_oid = index.write_tree()?;
        let tree = self.repo.find_tree(tree_oid)?;
        let sig = self.signature()?;
        let parent = self.repo.head()?.peel_to_commit()?;

        let full_message = format!("experiment: {message}");
        let oid = self
            .repo
            .commit(Some("HEAD"), &sig, &sig, &full_message, &tree, &[&parent])?;

        Ok(oid.to_string()[..7].to_string())
    }

    /// Revert the last commit (safe revert: creates a revert commit).
    pub fn revert_head(&self) -> Result<()> {
        let head_commit = self.repo.head()?.peel_to_commit()?;
        self.repo.revert(&head_commit, None)?;

        // Complete the revert with a commit
        let mut index = self.repo.index()?;
        let tree_oid = index.write_tree()?;
        let tree = self.repo.find_tree(tree_oid)?;
        let sig = self.signature()?;
        let parent = self.repo.head()?.peel_to_commit()?;

        let msg = format!(
            "Revert \"{}\"",
            head_commit.summary().unwrap_or("experiment")
        );
        self.repo
            .commit(Some("HEAD"), &sig, &sig, &msg, &tree, &[&parent])?;

        // Clean up revert state
        self.repo.cleanup_state()?;
        Ok(())
    }

    /// Hard reset to HEAD~1 (only for dedicated experiment branches).
    pub fn hard_reset_head(&self) -> Result<()> {
        let head = self.repo.head()?.peel_to_commit()?;
        let parent = head.parent(0).context("HEAD has no parent")?;
        self.repo
            .reset(parent.as_object(), git2::ResetType::Hard, None)?;
        Ok(())
    }

    /// Get recent commit log (one-line format).
    pub fn log_oneline(&self, count: usize) -> Result<Vec<String>> {
        let mut revwalk = self.repo.revwalk()?;
        revwalk.push_head()?;

        let mut lines = Vec::new();
        for (i, oid_result) in revwalk.enumerate() {
            if i >= count {
                break;
            }
            let oid = oid_result?;
            let commit = self.repo.find_commit(oid)?;
            let short = &oid.to_string()[..7];
            let summary = commit.summary().unwrap_or("");
            lines.push(format!("{short} {summary}"));
        }

        Ok(lines)
    }

    /// Get the diff of the last commit.
    pub fn diff_last(&self) -> Result<String> {
        let head = self.repo.head()?.peel_to_commit()?;
        let parent = head.parent(0).context("HEAD has no parent")?;

        let parent_tree = parent.tree()?;
        let head_tree = head.tree()?;

        let diff = self
            .repo
            .diff_tree_to_tree(Some(&parent_tree), Some(&head_tree), None)?;

        let mut buf = Vec::new();
        diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
            buf.extend_from_slice(line.content());
            true
        })?;

        String::from_utf8(buf).context("Diff contains invalid UTF-8")
    }

    fn signature(&self) -> Result<Signature<'_>> {
        self.repo
            .signature()
            .or_else(|_| Signature::now("autoresearch", "autoresearch@localhost"))
            .context("Failed to create git signature")
    }
}

fn is_owned_artifact(path: &str) -> bool {
    // Check if path is under autoresearch-results/
    if path.starts_with("autoresearch-results/") || path == "autoresearch-results" {
        return true;
    }
    // Check if path basename matches owned artifacts
    let basename = Path::new(path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("");
    OWNED_ARTIFACTS.contains(&basename)
        || OWNED_DIRS.iter().any(|d| path.starts_with(d))
        || path.starts_with(".codex-autoresearch/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_owned_artifact() {
        assert!(is_owned_artifact("autoresearch-results/results.tsv"));
        assert!(is_owned_artifact("autoresearch-results/state.json"));
        assert!(is_owned_artifact(".codex-autoresearch/pointer.json"));
        assert!(!is_owned_artifact("src/main.rs"));
        assert!(!is_owned_artifact("package.json"));
    }
}
