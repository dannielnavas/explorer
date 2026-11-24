use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::fs::error::FsError;

/// Modo de operación del portapapeles de archivos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardMode {
    Copy,
    Cut,
}

/// Estado del portapapeles interno y compatible con `x-special/gnome-copied-files`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardState {
    pub mode: ClipboardMode,
    pub paths: Vec<PathBuf>,
}

/// Estrategia de resolución ante colisiones de nombre en destino.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollisionPolicy {
    Overwrite,
    Rename,
    Skip,
}

/// Formatea el contenido del portapapeles según el estándar GNOME/Nautilus (`x-special/gnome-copied-files`).
pub fn format_gnome_copied_files(mode: ClipboardMode, paths: &[PathBuf]) -> String {
    let header = match mode {
        ClipboardMode::Copy => "copy",
        ClipboardMode::Cut => "cut",
    };

    let mut out = String::from(header);
    for path in paths {
        out.push('\n');
        out.push_str("file://");
        out.push_str(&percent_encode_path(&path.to_string_lossy()));
    }
    out
}

/// Parsea una carga `x-special/gnome-copied-files` en un `ClipboardState`.
pub fn parse_gnome_copied_files(payload: &str) -> Option<ClipboardState> {
    let mut lines = payload.lines();
    let mode = match lines.next()?.trim() {
        "copy" => ClipboardMode::Copy,
        "cut" => ClipboardMode::Cut,
        _ => return None,
    };

    let paths: Vec<PathBuf> = lines
        .filter_map(|line| {
            let trimmed = line.trim();
            let raw = trimmed.strip_prefix("file://")?;
            Some(PathBuf::from(percent_decode_path(raw)))
        })
        .collect();

    if paths.is_empty() {
        None
    } else {
        Some(ClipboardState { mode, paths })
    }
}

/// Resuelve la ruta final de destino aplicando la política de colisión.
pub fn resolve_destination(
    dest_dir: &Path,
    file_name: &OsStr,
    policy: CollisionPolicy,
) -> Result<Option<PathBuf>, FsError> {
    let candidate = dest_dir.join(file_name);
    if !candidate.exists() {
        return Ok(Some(candidate));
    }

    match policy {
        CollisionPolicy::Overwrite => Ok(Some(candidate)),
        CollisionPolicy::Skip => Ok(None),
        CollisionPolicy::Rename => {
            let path = Path::new(file_name);
            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| file_name.to_string_lossy().into_owned());
            let ext = path
                .extension()
                .map(|e| format!(".{}", e.to_string_lossy()))
                .unwrap_or_default();

            for idx in 1..10_000u32 {
                let renamed = dest_dir.join(format!("{stem} ({idx}){ext}"));
                if !renamed.exists() {
                    return Ok(Some(renamed));
                }
            }
            Err(FsError::AlreadyExists(candidate))
        }
    }
}

/// Pega (copia o mueve) todas las rutas del portapapeles en `dest_dir`.
pub async fn paste_entries(
    clipboard: &ClipboardState,
    dest_dir: &Path,
    policy: CollisionPolicy,
) -> Result<Vec<PathBuf>, FsError> {
    let mut created = Vec::with_capacity(clipboard.paths.len());

    for src in &clipboard.paths {
        if !src.exists() {
            return Err(FsError::NotFound(src.clone()));
        }
        let Some(file_name) = src.file_name() else {
            continue;
        };

        let effective_policy = if src.parent() == Some(dest_dir) && clipboard.mode == ClipboardMode::Copy {
            CollisionPolicy::Rename
        } else {
            policy
        };

        let Some(dest) = resolve_destination(dest_dir, file_name, effective_policy)? else {
            continue;
        };

        if src == &dest {
            continue;
        }

        match clipboard.mode {
            ClipboardMode::Copy => copy_entry(src, &dest).await?,
            ClipboardMode::Cut => move_entry(src, &dest).await?,
        }
        created.push(dest);
    }

    Ok(created)
}

/// Copia un archivo o directorio (recursivamente) hacia `dest`.
pub async fn copy_entry(src: &Path, dest: &Path) -> Result<(), FsError> {
    let src_owned = src.to_path_buf();
    let dest_owned = dest.to_path_buf();

    tokio::task::spawn_blocking(move || copy_recursive_sync(&src_owned, &dest_owned))
        .await
        .map_err(|err| FsError::io(dest, std::io::Error::other(err.to_string())))?
}

fn copy_recursive_sync(src: &Path, dest: &Path) -> Result<(), FsError> {
    let meta = std::fs::symlink_metadata(src).map_err(|e| FsError::io(src, e))?;
    if meta.is_dir() {
        std::fs::create_dir_all(dest).map_err(|e| FsError::io(dest, e))?;
        for entry_res in std::fs::read_dir(src).map_err(|e| FsError::io(src, e))? {
            let entry = entry_res.map_err(|e| FsError::io(src, e))?;
            let child_src = entry.path();
            let child_dest = dest.join(entry.file_name());
            copy_recursive_sync(&child_src, &child_dest)?;
        }
    } else {
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| FsError::io(parent, e))?;
        }
        std::fs::copy(src, dest).map_err(|e| FsError::io(dest, e))?;
    }
    Ok(())
}

/// Mueve un archivo o directorio. Si cruza particiones, recurre a copia + borrado.
pub async fn move_entry(src: &Path, dest: &Path) -> Result<(), FsError> {
    match tokio::fs::rename(src, dest).await {
        Ok(()) => Ok(()),
        Err(_) => {
            copy_entry(src, dest).await?;
            let src_owned = src.to_path_buf();
            tokio::task::spawn_blocking(move || {
                if src_owned.is_dir() {
                    std::fs::remove_dir_all(&src_owned)
                } else {
                    std::fs::remove_file(&src_owned)
                }
                .map_err(|e| FsError::io(&src_owned, e))
            })
            .await
            .map_err(|err| FsError::io(src, std::io::Error::other(err.to_string())))?
        }
    }
}

/// Renombra un archivo o carpeta dentro de su mismo directorio padre.
pub async fn rename_entry(path: &Path, new_name: &str) -> Result<PathBuf, FsError> {
    let trimmed = new_name.trim();
    if trimmed.is_empty() || trimmed.contains('/') || trimmed.contains('\0') {
        return Err(FsError::InvalidName(new_name.to_string()));
    }

    let parent = path
        .parent()
        .ok_or_else(|| FsError::InvalidName("no se puede renombrar la raíz".to_string()))?;
    let dest = parent.join(trimmed);

    if dest.exists() && dest != path {
        return Err(FsError::AlreadyExists(dest));
    }

    tokio::fs::rename(path, &dest)
        .await
        .map_err(|err| FsError::io(&dest, err))?;
    Ok(dest)
}

/// Crea una nueva carpeta en `parent`.
pub async fn create_folder(parent: &Path, name: &str) -> Result<PathBuf, FsError> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.contains('/') || trimmed.contains('\0') {
        return Err(FsError::InvalidName(name.to_string()));
    }
    let target = parent.join(trimmed);
    if target.exists() {
        return Err(FsError::AlreadyExists(target));
    }
    tokio::fs::create_dir(&target)
        .await
        .map_err(|err| FsError::io(&target, err))?;
    Ok(target)
}

/// Envía las rutas indicadas a la papelera del sistema (FreeDesktop Trash / macOS Trash).
pub async fn move_to_trash(paths: Vec<PathBuf>) -> Result<(), FsError> {
    if paths.is_empty() {
        return Ok(());
    }
    tokio::task::spawn_blocking(move || {
        trash::delete_all(&paths).map_err(|err| FsError::Trash(err.to_string()))
    })
    .await
    .map_err(|err| FsError::Trash(err.to_string()))?
}

fn percent_encode_path(input: &str) -> String {
    let mut encoded = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                encoded.push(byte as char);
            }
            _ => {
                encoded.push_str(&format!("%{byte:02X}"));
            }
        }
    }
    encoded
}

fn percent_decode_path(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
                if let Ok(val) = u8::from_str_radix(hex, 16) {
                    out.push(val);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
