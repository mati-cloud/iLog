//! File-tailing provider.
//!
//! Sources are glob patterns, re-expanded on an interval rather than once at
//! startup. A one-shot expansion is only correct for static paths like
//! `/var/log/gitlab/`; under `/var/log/pods` the set of matching files turns over
//! continuously as pods are scheduled and deleted, so anything created after
//! boot would never be tailed and nothing would say so.

use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, AsyncSeekExt, BufReader};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use super::LogProvider;
use crate::parser::{build_parser, LogParser, ParserConfig, SourceConfig};
use crate::tcp_sender::LogEntry;

/// How long a tailer waits before checking a quiet file again.
///
/// Polling replaces the previous per-file `notify` watcher, which spawned an OS
/// thread and a full Tokio runtime for every path. At GitLab's handful of files
/// that was merely wasteful; with re-globbing over `/var/log/pods` it would mean
/// hundreds of runtimes. Polling also handles rotation and truncation, which a
/// watcher pinned to a single inode does not.
const POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Consecutive misses before a tailer concludes its file is gone.
///
/// Rotation briefly unlinks or replaces the path, so a single failed stat is not
/// proof of deletion. At [`POLL_INTERVAL`] this is ~5s of grace.
const MISSING_TOLERANCE: u32 = 20;

pub struct FileProvider {
    parser_config: Arc<ParserConfig>,
}

impl FileProvider {
    pub fn new(parser_config: ParserConfig) -> Self {
        Self {
            parser_config: Arc::new(parser_config),
        }
    }

    /// Expand a source's glob, dropping anything matched by `exclude_paths`.
    fn expand(source: &SourceConfig) -> Vec<PathBuf> {
        let excludes: Vec<glob::Pattern> = source
            .exclude_paths
            .iter()
            .filter_map(|p| match glob::Pattern::new(p) {
                Ok(pat) => Some(pat),
                Err(e) => {
                    error!("Invalid exclude pattern {}: {}", p, e);
                    None
                }
            })
            .collect();

        match glob::glob(&source.path) {
            Ok(paths) => paths
                .filter_map(Result::ok)
                .filter(|p| p.is_file())
                .filter(|p| !excludes.iter().any(|pat| pat.matches_path(p)))
                .collect(),
            Err(e) => {
                error!("Invalid glob pattern {}: {}", source.path, e);
                Vec::new()
            }
        }
    }
}

/// Read new lines from `reader`, parse them, and forward them.
///
/// Returns `Ok(false)` when the channel closed, which means the sender is gone
/// and the tailer should stop.
async fn drain_lines(
    reader: &mut BufReader<File>,
    parser: &dyn LogParser,
    service_name: &str,
    tx: &mpsc::Sender<LogEntry>,
) -> Result<bool> {
    let mut line = String::new();
    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line).await?;
        if bytes_read == 0 {
            return Ok(true);
        }

        // A line without a trailing newline is a partial write: rewind so it is
        // re-read once the writer finishes it, otherwise the record is split.
        if !line.ends_with('\n') {
            let pos = reader.stream_position().await?;
            reader
                .seek(std::io::SeekFrom::Start(
                    pos.saturating_sub(bytes_read as u64),
                ))
                .await?;
            return Ok(true);
        }

        let log_text = line.trim_end_matches(['\n', '\r']);
        if log_text.is_empty() {
            continue;
        }

        // `None` from a CRI parser means a partial record was buffered and the
        // logical line is not complete yet -- not a failure, just no event.
        let Some(attrs) = parser.parse(log_text) else {
            continue;
        };

        let message = attrs
            .get("message")
            .or_else(|| attrs.get("msg"))
            .or_else(|| attrs.get("body"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| log_text.to_string());

        let entry = LogEntry {
            timestamp: chrono::Utc::now(),
            level: attrs
                .get("severity")
                .or_else(|| attrs.get("level"))
                .and_then(|v| v.as_str())
                .unwrap_or("INFO")
                .to_string(),
            service: service_name.to_string(),
            message,
            attributes: Some(serde_json::to_value(attrs).unwrap_or(serde_json::json!({}))),
        };

        if tx.send(entry).await.is_err() {
            return Ok(false);
        }
    }
}

/// Tail one file until it disappears or the channel closes.
///
/// `from_start` distinguishes the two discovery cases. Files present when the
/// agent boots are tailed from the end, so a restart does not re-ingest history.
/// Files found by a later sweep are new -- a pod that just started -- so they are
/// read from position 0 and the discovery interval costs no lines.
async fn tail_file(
    path: PathBuf,
    parser: Box<dyn LogParser>,
    service_name: String,
    from_start: bool,
    tx: mpsc::Sender<LogEntry>,
) -> Result<()> {
    let file = File::open(&path)
        .await
        .with_context(|| format!("Failed to open file: {}", path.display()))?;

    let mut reader = BufReader::new(file);
    if !from_start {
        reader.seek(std::io::SeekFrom::End(0)).await?;
    }

    info!(
        "Tailing {} (from {})",
        path.display(),
        if from_start { "start" } else { "end" }
    );

    let mut missing = 0u32;

    loop {
        if !drain_lines(&mut reader, parser.as_ref(), &service_name, &tx).await? {
            return Ok(());
        }

        tokio::time::sleep(POLL_INTERVAL).await;

        // Rotation and deletion both surface here. Comparing the file's current
        // length against our offset catches truncate-in-place (copytruncate);
        // comparing inodes would be needed for renames, but the re-glob sweep
        // picks the replacement up as a new path anyway.
        match tokio::fs::metadata(&path).await {
            Ok(meta) => {
                missing = 0;
                let pos = reader.stream_position().await?;
                if meta.len() < pos {
                    info!("{} was truncated, resuming from start", path.display());
                    reader.seek(std::io::SeekFrom::Start(0)).await?;
                }
            }
            Err(_) => {
                missing += 1;
                if missing >= MISSING_TOLERANCE {
                    info!("{} is gone, stopping tailer", path.display());
                    return Ok(());
                }
            }
        }
    }
}

#[async_trait::async_trait]
impl LogProvider for FileProvider {
    async fn start(&self, tx: mpsc::Sender<LogEntry>) -> Result<()> {
        let interval = Duration::from_secs(self.parser_config.discovery_interval_secs.max(1));
        info!(
            "Starting FileProvider with {} sources, rediscovering every {}s",
            self.parser_config.sources.len(),
            interval.as_secs()
        );

        // Validate every pattern once up front so a bad regex is reported at
        // startup rather than on each sweep.
        for source in &self.parser_config.sources {
            if let Err(e) = build_parser(&source.parser) {
                error!("Source {} has an invalid parser: {}", source.name, e);
            }
        }

        // Shared so a tailer can remove its own path on exit, which is what lets
        // a recreated path (same name, new pod) be picked up again.
        let active: Arc<Mutex<HashSet<PathBuf>>> = Arc::new(Mutex::new(HashSet::new()));
        let mut first_sweep = true;

        loop {
            let mut discovered = 0usize;

            for source in &self.parser_config.sources {
                for path in Self::expand(source) {
                    {
                        let mut guard = active.lock().unwrap();
                        if !guard.insert(path.clone()) {
                            continue;
                        }
                    }

                    // One parser per file: CriParser holds reassembly state, and
                    // sharing it across files would splice unrelated lines.
                    let parser = match build_parser(&source.parser) {
                        Ok(p) => p,
                        Err(_) => {
                            active.lock().unwrap().remove(&path);
                            continue;
                        }
                    };

                    discovered += 1;
                    let tx = tx.clone();
                    let service_name = source.name.clone();
                    let active = active.clone();
                    let from_start = !first_sweep;
                    let spawn_path = path.clone();

                    tokio::spawn(async move {
                        if let Err(e) = tail_file(
                            spawn_path.clone(),
                            parser,
                            service_name,
                            from_start,
                            tx,
                        )
                        .await
                        {
                            error!("Error tailing {}: {}", spawn_path.display(), e);
                        }
                        // Reap so the path is eligible for rediscovery.
                        active.lock().unwrap().remove(&spawn_path);
                    });
                }
            }

            if first_sweep && discovered == 0 {
                warn!(
                    "No files matched any source pattern yet; will keep looking every {}s",
                    interval.as_secs()
                );
            } else if discovered > 0 {
                debug!("Discovered {} new file(s)", discovered);
            }

            first_sweep = false;
            tokio::time::sleep(interval).await;
        }
    }

    fn name(&self) -> &str {
        "file"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{ParserType, SourceConfig};
    use std::io::Write;

    fn source(path: String, exclude: Vec<String>) -> SourceConfig {
        SourceConfig {
            name: "test".to_string(),
            path,
            exclude_paths: exclude,
            parser: ParserType::Cri {
                inner: None,
                fields: vec![],
            },
        }
    }

    /// The bug this replaced: `glob` ran once at startup, so a file created later
    /// was never tailed. Under /var/log/pods that is every pod scheduled after
    /// the agent booted.
    #[tokio::test]
    async fn files_created_after_startup_are_discovered() {
        let dir = std::env::temp_dir().join(format!("ilog-glob-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let pattern = format!("{}/*.log", dir.display());

        // Nothing yet: the pre-fix code logged a warning and gave up here.
        assert_eq!(FileProvider::expand(&source(pattern.clone(), vec![])).len(), 0);

        let mut f = std::fs::File::create(dir.join("late.log")).unwrap();
        writeln!(f, "2026-08-06T10:00:00Z stdout F {{\"msg\":\"hi\"}}").unwrap();

        // A later sweep must see it.
        let found = FileProvider::expand(&source(pattern, vec![]));
        assert_eq!(found.len(), 1);
        assert!(found[0].ends_with("late.log"));

        std::fs::remove_dir_all(&dir).ok();
    }

    /// Guards the feedback loop: the agent tailing its own backend's log means
    /// every ingest produces a line that is itself ingested.
    #[tokio::test]
    async fn excluded_paths_are_dropped() {
        let dir = std::env::temp_dir().join(format!("ilog-excl-{}", std::process::id()));
        let pods = dir.join("var/log/pods");
        std::fs::create_dir_all(pods.join("ilog_backend-abc/backend")).unwrap();
        std::fs::create_dir_all(pods.join("default_app-xyz/app")).unwrap();
        std::fs::write(pods.join("ilog_backend-abc/backend/0.log"), "x\n").unwrap();
        std::fs::write(pods.join("default_app-xyz/app/0.log"), "x\n").unwrap();

        let found = FileProvider::expand(&source(
            format!("{}/*/*/*.log", pods.display()),
            vec![format!("{}/ilog_*/**", pods.display())],
        ));

        assert_eq!(found.len(), 1, "only the non-ilog pod should remain");
        assert!(found[0].to_string_lossy().contains("default_app-xyz"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn invalid_exclude_pattern_does_not_drop_everything() {
        let dir = std::env::temp_dir().join(format!("ilog-badexcl-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.log"), "x\n").unwrap();

        // A malformed exclude must be ignored, not silently match nothing/everything.
        let found = FileProvider::expand(&source(
            format!("{}/*.log", dir.display()),
            vec!["[".to_string()],
        ));
        assert_eq!(found.len(), 1);

        std::fs::remove_dir_all(&dir).ok();
    }
}
