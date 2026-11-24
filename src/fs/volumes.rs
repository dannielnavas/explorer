use std::collections::HashSet;
use std::path::PathBuf;

use sysinfo::Disks;

use crate::core::entry::format_bytes;

/// Información en tiempo real de un volumen o partición montada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumeInfo {
    pub label: String,
    pub mount_point: PathBuf,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub file_system: String,
    pub is_removable: bool,
}

impl VolumeInfo {
    /// Fracción usada entre `0.0` y `1.0`.
    pub fn used_fraction(&self) -> f64 {
        if self.total_bytes == 0 {
            return 0.0;
        }
        let used = self.total_bytes.saturating_sub(self.available_bytes);
        (used as f64 / self.total_bytes as f64).clamp(0.0, 1.0)
    }

    /// Resumen estilo tarjeta inferior (`34 GB Free of 250 GB`).
    pub fn free_summary(&self) -> String {
        format!(
            "{} Free of {}",
            format_bytes(self.available_bytes),
            format_bytes(self.total_bytes)
        )
    }
}

/// Acceso rápido XDG del sistema.
#[derive(Debug, Clone)]
pub struct QuickLocation {
    pub label: &'static str,
    pub icon_name: &'static str,
    pub path: PathBuf,
}

/// Devuelve las carpetas estándar XDG disponibles para el usuario actual.
pub fn xdg_quick_locations() -> Vec<QuickLocation> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    let candidates = [
        ("Desktop", "user-desktop-symbolic", dirs::desktop_dir()),
        ("Downloads", "folder-download-symbolic", dirs::download_dir()),
        ("Documents", "folder-documents-symbolic", dirs::document_dir()),
        ("Pictures", "folder-pictures-symbolic", dirs::picture_dir()),
        ("Music", "folder-music-symbolic", dirs::audio_dir()),
        ("Videos", "folder-videos-symbolic", dirs::video_dir()),
    ];

    candidates
        .into_iter()
        .map(|(label, icon_name, opt_path)| QuickLocation {
            label,
            icon_name,
            path: opt_path.unwrap_or_else(|| home.join(label)),
        })
        .collect()
}

/// Lista los volúmenes físicos y particiones relevantes del sistema (Linux/CachyOS y macOS).
pub fn list_volumes() -> Vec<VolumeInfo> {
    let disks = Disks::new_with_refreshed_list();
    let mut seen_mounts = HashSet::new();
    let mut volumes = Vec::new();

    for disk in disks.list() {
        let mount = disk.mount_point().to_path_buf();
        let total = disk.total_space();
        if total == 0 || !seen_mounts.insert(mount.clone()) {
            continue;
        }

        let fs_name = disk.file_system().to_string_lossy().to_string();
        if is_pseudo_filesystem(&fs_name, &mount) {
            continue;
        }

        let raw_name = disk.name().to_string_lossy();
        let label = format_volume_label(&raw_name, &mount);

        volumes.push(VolumeInfo {
            label,
            mount_point: mount,
            total_bytes: total,
            available_bytes: disk.available_space(),
            file_system: fs_name,
            is_removable: disk.is_removable(),
        });
    }

    volumes.sort_by(|a, b| a.mount_point.cmp(&b.mount_point));
    volumes
}

fn is_pseudo_filesystem(fs_name: &str, mount: &std::path::Path) -> bool {
    let fs_lower = fs_name.to_ascii_lowercase();
    if matches!(
        fs_lower.as_str(),
        "tmpfs" | "devtmpfs" | "squashfs" | "overlay" | "proc" | "sysfs" | "cgroup2" | "autofs"
    ) {
        return true;
    }

    let mount_str = mount.to_string_lossy();
    mount_str.starts_with("/System/Volumes/")
        || mount_str.starts_with("/boot")
        || mount_str.starts_with("/snap/")
        || mount_str.starts_with("/run/")
}

fn format_volume_label(raw_name: &str, mount: &std::path::Path) -> String {
    if mount == std::path::Path::new("/") {
        return "OS (/)".to_string();
    }
    if let Some(folder) = mount.file_name().and_then(|n| n.to_str()) {
        if !folder.is_empty() {
            return format!("{folder} ({})", mount.display());
        }
    }
    if !raw_name.is_empty() {
        raw_name.to_string()
    } else {
        mount.display().to_string()
    }
}
