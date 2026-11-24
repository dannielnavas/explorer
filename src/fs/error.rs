use std::path::PathBuf;

use thiserror::Error;

/// Errores tipados de la capa de sistema de archivos.
#[derive(Debug, Error)]
pub enum FsError {
    #[error("error de E/S en '{}': {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("la ruta no existe: '{}'", .0.display())]
    NotFound(PathBuf),

    #[error("ya existe un elemento en '{}'", .0.display())]
    AlreadyExists(PathBuf),

    #[error("nombre inválido: {0}")]
    InvalidName(String),

    #[error("no se pudo mover a la papelera: {0}")]
    Trash(String),

    #[error("error al observar cambios en disco: {0}")]
    Watch(String),

    #[error("no se pudo generar la miniatura: {0}")]
    Thumbnail(String),
}

impl FsError {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}
