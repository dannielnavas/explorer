use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::core::entry::{SortKey, SortOrder, ViewMode};

/// Configuración persistente de la aplicación almacenada en `$XDG_CONFIG_HOME/explorer/config.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub default_path: PathBuf,
    pub icon_size: f64,
    pub show_hidden: bool,
    pub sort_key: SortKey,
    pub sort_order: SortOrder,
    pub view_mode: ViewMode,
}

impl Default for AppConfig {
    fn default() -> Self {
        let default_path = dirs::document_dir()
            .filter(|p| p.exists())
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("/"));

        Self {
            default_path,
            icon_size: 96.0,
            show_hidden: false,
            sort_key: SortKey::Name,
            sort_order: SortOrder::Ascending,
            view_mode: ViewMode::Grid,
        }
    }
}

impl AppConfig {
    /// Ruta del archivo de configuración XDG.
    pub fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|dir| dir.join("explorer").join("config.toml"))
    }

    /// Carga la configuración desde disco o devuelve valores por defecto si no existe o falla.
    pub fn load() -> Self {
        let Some(path) = Self::config_path() else {
            return Self::default();
        };

        let Ok(contents) = std::fs::read_to_string(path) else {
            return Self::default();
        };

        toml::from_str::<Self>(&contents)
            .map(|mut cfg| {
                cfg.icon_size = cfg.icon_size.clamp(64.0, 256.0);
                if !cfg.default_path.exists() {
                    cfg.default_path = Self::default().default_path;
                }
                cfg
            })
            .unwrap_or_default()
    }

    /// Persiste la configuración actual en disco (fallo silencioso si no hay permisos).
    pub fn save(&self) {
        let Some(path) = Self::config_path() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(serialized) = toml::to_string_pretty(self) {
            let _ = std::fs::write(path, serialized);
        }
    }
}
