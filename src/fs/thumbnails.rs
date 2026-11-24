use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::fs::error::FsError;

/// Devuelve la ruta de caché FreeDesktop (`$XDG_CACHE_HOME/thumbnails/normal/<md5>.png`).
pub fn thumbnail_cache_path(file_path: &Path) -> Option<PathBuf> {
    let cache_root = dirs::cache_dir()?;
    let canonical = std::fs::canonicalize(file_path).unwrap_or_else(|_| file_path.to_path_buf());
    let uri = format!("file://{}", canonical.to_string_lossy());
    let digest = md5::compute(uri.as_bytes());
    let filename = format!("{digest:x}.png");
    Some(cache_root.join("thumbnails").join("normal").join(filename))
}

/// Comprueba si existe una miniatura vigente (más reciente que la modificación del archivo fuente).
pub fn existing_valid_thumbnail(file_path: &Path, source_mtime: Option<SystemTime>) -> Option<PathBuf> {
    let thumb_path = thumbnail_cache_path(file_path)?;
    let thumb_meta = std::fs::metadata(&thumb_path).ok()?;
    let thumb_mtime = thumb_meta.modified().ok()?;
    if let Some(src_time) = source_mtime {
        if thumb_mtime < src_time {
            return None;
        }
    }
    Some(thumb_path)
}

/// Genera y almacena en caché XDG una miniatura PNG de 128x128 para imágenes soportadas.
pub async fn generate_thumbnail(file_path: PathBuf) -> Result<PathBuf, FsError> {
    let Some(thumb_path) = thumbnail_cache_path(&file_path) else {
        return Err(FsError::Thumbnail(
            "no se pudo resolver $XDG_CACHE_HOME".to_string(),
        ));
    };

    let source_mtime = tokio::fs::metadata(&file_path)
        .await
        .ok()
        .and_then(|m| m.modified().ok());

    if let Some(valid) = existing_valid_thumbnail(&file_path, source_mtime) {
        return Ok(valid);
    }

    tokio::task::spawn_blocking(move || -> Result<PathBuf, FsError> {
        if let Some(parent) = thumb_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| FsError::io(parent, e))?;
        }

        let img = image::open(&file_path).map_err(|e| FsError::Thumbnail(e.to_string()))?;
        let thumb = img.thumbnail(128, 128);

        let tmp_path = thumb_path.with_extension("png.tmp");
        thumb
            .save_with_format(&tmp_path, image::ImageFormat::Png)
            .map_err(|e| FsError::Thumbnail(e.to_string()))?;
        std::fs::rename(&tmp_path, &thumb_path).map_err(|e| FsError::io(&thumb_path, e))?;

        Ok(thumb_path)
    })
    .await
    .map_err(|e| FsError::Thumbnail(e.to_string()))?
}
