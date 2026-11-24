use std::path::PathBuf;

use tokio::sync::mpsc;

use crate::core::entry::FileEntry;
use crate::fs::error::FsError;

/// Lee un directorio completo de forma asíncrona sin bloquear el hilo de UI.
pub async fn read_directory(path: PathBuf, show_hidden: bool) -> Result<Vec<FileEntry>, FsError> {
    let mut read_dir = tokio::fs::read_dir(&path)
        .await
        .map_err(|err| FsError::io(&path, err))?;

    let mut entries = Vec::new();
    while let Some(dir_entry) = read_dir
        .next_entry()
        .await
        .map_err(|err| FsError::io(&path, err))?
    {
        let entry_path = dir_entry.path();
        let file_name = dir_entry.file_name();
        if !show_hidden && file_name.to_string_lossy().starts_with('.') {
            continue;
        }

        let symlink_meta = match tokio::fs::symlink_metadata(&entry_path).await {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        let is_symlink = symlink_meta.file_type().is_symlink();
        let metadata = if is_symlink {
            tokio::fs::metadata(&entry_path)
                .await
                .unwrap_or(symlink_meta)
        } else {
            symlink_meta
        };

        entries.push(FileEntry::from_metadata(entry_path, &metadata, is_symlink));
    }

    Ok(entries)
}

/// Transmite las entradas de un directorio en lotes por un canal MPSC de Tokio.
/// Permite abrir directorios con >100,000 archivos de forma progresiva e instantánea.
pub fn stream_directory(
    path: PathBuf,
    show_hidden: bool,
    batch_size: usize,
) -> mpsc::Receiver<Result<Vec<FileEntry>, FsError>> {
    let (tx, rx) = mpsc::channel(16);
    let effective_batch = batch_size.max(32);

    tokio::spawn(async move {
        let mut read_dir = match tokio::fs::read_dir(&path).await {
            Ok(rd) => rd,
            Err(err) => {
                let _ = tx.send(Err(FsError::io(&path, err))).await;
                return;
            }
        };

        let mut batch = Vec::with_capacity(effective_batch);

        loop {
            match read_dir.next_entry().await {
                Ok(Some(dir_entry)) => {
                    let entry_path = dir_entry.path();
                    if !show_hidden && dir_entry.file_name().to_string_lossy().starts_with('.') {
                        continue;
                    }

                    let symlink_meta = match tokio::fs::symlink_metadata(&entry_path).await {
                        Ok(meta) => meta,
                        Err(_) => continue,
                    };
                    let is_symlink = symlink_meta.file_type().is_symlink();
                    let metadata = if is_symlink {
                        tokio::fs::metadata(&entry_path)
                            .await
                            .unwrap_or(symlink_meta)
                    } else {
                        symlink_meta
                    };

                    batch.push(FileEntry::from_metadata(entry_path, &metadata, is_symlink));

                    if batch.len() >= effective_batch {
                        let to_send = std::mem::replace(&mut batch, Vec::with_capacity(effective_batch));
                        if tx.send(Ok(to_send)).await.is_err() {
                            return;
                        }
                    }
                }
                Ok(None) => {
                    if !batch.is_empty() {
                        let _ = tx.send(Ok(batch)).await;
                    }
                    break;
                }
                Err(err) => {
                    let _ = tx.send(Err(FsError::io(&path, err))).await;
                    break;
                }
            }
        }
    });

    rx
}
