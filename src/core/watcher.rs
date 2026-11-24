use std::path::{Path, PathBuf};

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;

use crate::fs::error::FsError;

/// Observador de cambios en el sistema de archivos (inotify en Linux, FSEvents en macOS).
pub struct FileSystemWatcher {
    watcher: RecommendedWatcher,
    watched_dir: Option<PathBuf>,
}

impl FileSystemWatcher {
    /// Crea un nuevo observador que notifica cualquier cambio relevante al canal `tx`.
    pub fn new(tx: mpsc::UnboundedSender<PathBuf>) -> Result<Self, FsError> {
        let watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            if let Ok(event) = res {
                if event.kind.is_create() || event.kind.is_remove() || event.kind.is_modify() {
                    let target = event
                        .paths
                        .first()
                        .and_then(|p| p.parent().map(Path::to_path_buf))
                        .unwrap_or_default();
                    let _ = tx.send(target);
                }
            }
        })
        .map_err(|err| FsError::Watch(err.to_string()))?;

        Ok(Self {
            watcher,
            watched_dir: None,
        })
    }

    /// Cambia el directorio observado de forma no recursiva.
    pub fn watch(&mut self, dir: &Path) -> Result<(), FsError> {
        if self.watched_dir.as_deref() == Some(dir) {
            return Ok(());
        }

        if let Some(prev) = self.watched_dir.take() {
            let _ = self.watcher.unwatch(&prev);
        }

        self.watcher
            .watch(dir, RecursiveMode::NonRecursive)
            .map_err(|err| FsError::Watch(err.to_string()))?;
        self.watched_dir = Some(dir.to_path_buf());
        Ok(())
    }
}
