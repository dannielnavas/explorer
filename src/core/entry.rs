use std::cmp::Ordering;
use std::fs::Metadata;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

/// Clasificación semántica de un archivo para asignar iconos, badges y ordenación.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum FileKind {
    Directory,
    Pdf,
    Document,
    Image,
    Audio,
    Video,
    Archive,
    Code,
    Executable,
    Other,
}

/// Clave de ordenación de entradas en el directorio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortKey {
    Name,
    Modified,
    Size,
    Kind,
}

/// Dirección de ordenación.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortOrder {
    Ascending,
    Descending,
}

impl SortOrder {
    pub fn toggle(self) -> Self {
        match self {
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Ascending,
        }
    }
}

/// Modo de visualización de archivos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewMode {
    Grid,
    List,
}

/// Representación inmutable de una entrada del sistema de archivos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
    pub kind: FileKind,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub is_hidden: bool,
    pub is_symlink: bool,
    pub readonly: bool,
}

impl FileEntry {
    /// Construye un `FileEntry` a partir de su ruta y metadatos ya leídos.
    pub fn from_metadata(path: PathBuf, metadata: &Metadata, is_symlink: bool) -> Self {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string_lossy().into_owned());

        let is_hidden = name.starts_with('.');
        let kind = classify_path(&path, metadata);
        let size = if metadata.is_dir() { 0 } else { metadata.len() };
        let modified = metadata.modified().ok();
        let readonly = metadata.permissions().readonly();

        Self {
            name,
            path,
            kind,
            size,
            modified,
            is_hidden,
            is_symlink,
            readonly,
        }
    }

    pub fn is_dir(&self) -> bool {
        self.kind == FileKind::Directory
    }

    /// Tamaño legible para humanos (e.g., `1.4 MB`, `342 KB`).
    pub fn formatted_size(&self) -> String {
        if self.is_dir() {
            return "—".to_string();
        }
        format_bytes(self.size)
    }

    /// Fecha de modificación formateada en zona horaria local.
    pub fn formatted_modified(&self) -> String {
        match self.modified {
            Some(time) => {
                let dt: DateTime<Local> = time.into();
                dt.format("%Y-%m-%d %H:%M").to_string()
            }
            None => "—".to_string(),
        }
    }

    /// Nombre de icono simbólico/estándar de GNOME como respaldo.
    pub fn fallback_icon_name(&self) -> &'static str {
        match self.kind {
            FileKind::Directory => "folder-symbolic",
            FileKind::Pdf => "application-pdf-symbolic",
            FileKind::Document => "x-office-document-symbolic",
            FileKind::Image => "image-x-generic-symbolic",
            FileKind::Audio => "audio-x-generic-symbolic",
            FileKind::Video => "video-x-generic-symbolic",
            FileKind::Archive => "package-x-generic-symbolic",
            FileKind::Code => "text-x-generic-symbolic",
            FileKind::Executable => "application-x-executable-symbolic",
            FileKind::Other => "text-x-generic-symbolic",
        }
    }
}

/// Clasifica una ruta según sus metadatos y extensión.
pub fn classify_path(path: &Path, metadata: &Metadata) -> FileKind {
    if metadata.is_dir() {
        return FileKind::Directory;
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());

    match ext.as_deref() {
        Some("pdf") => FileKind::Pdf,
        Some("doc" | "docx" | "odt" | "rtf" | "txt" | "md" | "pages" | "xls" | "xlsx" | "csv" | "ppt" | "pptx") => {
            FileKind::Document
        }
        Some("png" | "jpg" | "jpeg" | "webp" | "gif" | "svg" | "bmp" | "ico" | "avif" | "heic") => {
            FileKind::Image
        }
        Some("mp3" | "flac" | "ogg" | "wav" | "m4a" | "opus" | "aac") => FileKind::Audio,
        Some("mp4" | "mkv" | "webm" | "avi" | "mov" | "flv") => FileKind::Video,
        Some("zip" | "tar" | "gz" | "xz" | "zst" | "bz2" | "7z" | "rar" | "deb" | "rpm") => {
            FileKind::Archive
        }
        Some(
            "rs" | "py" | "js" | "ts" | "tsx" | "jsx" | "c" | "cpp" | "h" | "hpp" | "go" | "java"
            | "kt" | "swift" | "sh" | "zsh" | "bash" | "toml" | "yaml" | "yml" | "json" | "html"
            | "css" | "sql" | "xml" | "nix",
        ) => FileKind::Code,
        _ => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if metadata.permissions().mode() & 0o111 != 0 {
                    return FileKind::Executable;
                }
            }
            FileKind::Other
        }
    }
}

/// Formatea bytes en unidades legibles (base 1024).
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64;
    let mut unit_idx = 0;
    while value >= 1024.0 && unit_idx < UNITS.len() - 1 {
        value /= 1024.0;
        unit_idx += 1;
    }
    if value >= 100.0 {
        format!("{value:.0} {}", UNITS[unit_idx])
    } else {
        format!("{value:.1} {}", UNITS[unit_idx])
    }
}

/// Ordena entradas manteniendo siempre los directorios antes que los archivos.
pub fn sort_entries(entries: &mut [FileEntry], key: SortKey, order: SortOrder) {
    entries.sort_by(|a, b| {
        let dir_cmp = b.is_dir().cmp(&a.is_dir());
        if dir_cmp != Ordering::Equal {
            return dir_cmp;
        }

        let primary = match key {
            SortKey::Name => compare_names(&a.name, &b.name),
            SortKey::Modified => a.modified.cmp(&b.modified),
            SortKey::Size => a.size.cmp(&b.size),
            SortKey::Kind => a.kind.cmp(&b.kind),
        };

        let ordered = match order {
            SortOrder::Ascending => primary,
            SortOrder::Descending => primary.reverse(),
        };

        if ordered == Ordering::Equal {
            compare_names(&a.name, &b.name)
        } else {
            ordered
        }
    });
}

fn compare_names(a: &str, b: &str) -> Ordering {
    a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase())
}
