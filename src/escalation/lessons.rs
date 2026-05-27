use anyhow::{Context, Result};
use chrono::Utc;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// A lesson extracted from iteration results.
#[derive(Debug, Clone)]
pub struct Lesson {
    pub timestamp: String,
    pub category: LessonCategory,
    pub strategy: String,
    pub outcome: LessonOutcome,
    pub context: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LessonCategory {
    Positive,  // Strategy that worked
    Negative,  // Strategy that consistently failed
    Strategic, // Pivot-level insight
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LessonOutcome {
    Success,
    Failure,
    Neutral,
}

impl Lesson {
    pub fn to_markdown(&self) -> String {
        let category = match self.category {
            LessonCategory::Positive => "✅",
            LessonCategory::Negative => "❌",
            LessonCategory::Strategic => "🔄",
        };
        let outcome = match self.outcome {
            LessonOutcome::Success => "worked",
            LessonOutcome::Failure => "failed",
            LessonOutcome::Neutral => "neutral",
        };

        format!(
            "- {} [{}] **{}** — {} ({})\n",
            category, self.timestamp, self.strategy, self.context, outcome
        )
    }
}

/// Lessons file manager.
pub struct LessonsLog {
    path: PathBuf,
}

impl LessonsLog {
    /// Open or create a lessons file.
    pub fn open_or_create(dir: &Path) -> Result<Self> {
        fs::create_dir_all(dir).context("Failed to create results directory")?;
        let path = dir.join("lessons.md");
        if !path.exists() {
            fs::write(&path, "# Autoresearch Lessons\n\n").context("Failed to create lessons")?;
        }
        Ok(Self { path })
    }

    /// Append a lesson.
    pub fn append(&self, lesson: &Lesson) -> Result<()> {
        let mut file = OpenOptions::new()
            .append(true)
            .open(&self.path)
            .context("Failed to open lessons file")?;
        write!(file, "{}", lesson.to_markdown()).context("Failed to write lesson")?;
        Ok(())
    }

    /// Read all lessons.
    pub fn read_all(&self) -> Result<Vec<String>> {
        let content = fs::read_to_string(&self.path).context("Failed to read lessons")?;
        Ok(content
            .lines()
            .filter(|l| l.starts_with("- "))
            .map(|l| l.to_string())
            .collect())
    }

    /// Search lessons for relevant strategies.
    pub fn search(&self, query: &str) -> Result<Vec<String>> {
        let all = self.read_all()?;
        let lower_query = query.to_lowercase();
        Ok(all
            .into_iter()
            .filter(|l| l.to_lowercase().contains(&lower_query))
            .collect())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Extract a positive lesson from a successful keep.
pub fn extract_keep_lesson(description: &str, metric_delta: &str) -> Lesson {
    Lesson {
        timestamp: Utc::now().format("%Y-%m-%d %H:%M").to_string(),
        category: LessonCategory::Positive,
        strategy: description.to_string(),
        outcome: LessonOutcome::Success,
        context: format!("delta: {metric_delta}"),
    }
}

/// Extract a strategic lesson from a pivot event.
pub fn extract_pivot_lesson(failed_strategy: &str, new_direction: &str) -> Lesson {
    Lesson {
        timestamp: Utc::now().format("%Y-%m-%d %H:%M").to_string(),
        category: LessonCategory::Strategic,
        strategy: format!("Pivoted from: {failed_strategy}"),
        outcome: LessonOutcome::Neutral,
        context: format!("New direction: {new_direction}"),
    }
}

/// Extract a negative lesson from repeated failures.
pub fn extract_failure_lesson(strategy: &str, failure_count: u32) -> Lesson {
    Lesson {
        timestamp: Utc::now().format("%Y-%m-%d %H:%M").to_string(),
        category: LessonCategory::Negative,
        strategy: strategy.to_string(),
        outcome: LessonOutcome::Failure,
        context: format!("failed {failure_count} consecutive times"),
    }
}
