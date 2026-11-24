use std::path::PathBuf;

use explorer::core::entry::{format_bytes, sort_entries, FileEntry, FileKind, SortKey, SortOrder};
use explorer::core::history::NavigationHistory;
use explorer::core::search::SearchEngine;
use explorer::fs::operations::{
    create_folder, format_gnome_copied_files, parse_gnome_copied_files, paste_entries,
    rename_entry, ClipboardMode, ClipboardState, CollisionPolicy,
};
use explorer::fs::reader::{read_directory, stream_directory};
use explorer::fs::volumes::list_volumes;

#[test]
fn history_should_navigate_back_forward_and_truncate_future() {
    let mut history = NavigationHistory::new(PathBuf::from("/"));
    assert!(!history.can_go_back());
    assert!(!history.can_go_forward());

    history.push(PathBuf::from("/home"));
    history.push(PathBuf::from("/home/user"));
    assert_eq!(history.current(), PathBuf::from("/home/user"));
    assert!(history.can_go_back());

    assert_eq!(history.go_back(), Some(PathBuf::from("/home").as_path()));
    assert!(history.can_go_forward());

    // Al navegar a una nueva ruta desde el medio, se descarta el historial futuro
    history.push(PathBuf::from("/etc"));
    assert!(!history.can_go_forward());
    assert_eq!(history.current(), PathBuf::from("/etc"));
}

#[test]
fn search_engine_should_rank_fuzzy_matches_and_preserve_all_on_empty_query() {
    let entries = vec![
        sample_entry("Project Agreement.pdf", FileKind::Pdf, 2048),
        sample_entry("Design Files", FileKind::Directory, 0),
        sample_entry("My Resume.docx", FileKind::Document, 1024),
    ];

    let mut engine = SearchEngine::new();
    assert_eq!(engine.filter(&entries, "   ").len(), 3);

    let matches = engine.filter(&entries, "proj");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].name, "Project Agreement.pdf");
}

#[test]
fn sort_entries_should_always_place_directories_first() {
    let mut entries = vec![
        sample_entry("zebra.txt", FileKind::Document, 500),
        sample_entry("alpha", FileKind::Directory, 0),
        sample_entry("beta.pdf", FileKind::Pdf, 100),
    ];

    sort_entries(&mut entries, SortKey::Name, SortOrder::Ascending);
    assert_eq!(entries[0].name, "alpha");
    assert_eq!(entries[1].name, "beta.pdf");
    assert_eq!(entries[2].name, "zebra.txt");
}

#[test]
fn format_bytes_should_render_human_readable_units() {
    assert_eq!(format_bytes(512), "512 B");
    assert_eq!(format_bytes(1536), "1.5 KB");
    assert_eq!(format_bytes(5 * 1024 * 1024), "5.0 MB");
}

#[test]
fn gnome_clipboard_payload_should_roundtrip_paths_with_spaces() {
    let paths = vec![
        PathBuf::from("/home/user/My Documents/file 1.pdf"),
        PathBuf::from("/home/user/notes.txt"),
    ];
    let payload = format_gnome_copied_files(ClipboardMode::Cut, &paths);
    let parsed = parse_gnome_copied_files(&payload).expect("valid payload");

    assert_eq!(parsed.mode, ClipboardMode::Cut);
    assert_eq!(parsed.paths, paths);
}

#[tokio::test]
async fn reader_and_file_operations_should_work_end_to_end() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();

    // Crear carpeta y archivos
    let folder = create_folder(root, "docs").await.expect("create folder");
    let file_a = folder.join("report.pdf");
    tokio::fs::write(&file_a, b"%PDF-1.4 sample").await.expect("write");
    tokio::fs::write(folder.join(".hidden"), b"secret").await.expect("write hidden");

    // Lectura sin ocultos vs con ocultos
    let visible = read_directory(folder.clone(), false).await.expect("read visible");
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].kind, FileKind::Pdf);

    let with_hidden = read_directory(folder.clone(), true).await.expect("read all");
    assert_eq!(with_hidden.len(), 2);

    // Lectura en streaming
    let mut rx = stream_directory(folder.clone(), true, 32);
    let mut streamed_count = 0usize;
    while let Some(batch) = rx.recv().await {
        streamed_count += batch.expect("batch ok").len();
    }
    assert_eq!(streamed_count, 2);

    // Copiar en el mismo directorio debe auto-renombrar sin sobrescribir
    let clip = ClipboardState {
        mode: ClipboardMode::Copy,
        paths: vec![file_a.clone()],
    };
    let pasted = paste_entries(&clip, &folder, CollisionPolicy::Rename)
        .await
        .expect("paste copy");
    assert_eq!(pasted.len(), 1);
    assert_eq!(
        pasted[0].file_name().and_then(|n| n.to_str()),
        Some("report (1).pdf")
    );

    // Renombrar
    let renamed = rename_entry(&pasted[0], "final.pdf")
        .await
        .expect("rename");
    assert!(renamed.exists());
    assert!(!pasted[0].exists());
}

#[test]
fn volume_monitor_should_detect_at_least_one_mounted_volume() {
    let volumes = list_volumes();
    assert!(!volumes.is_empty(), "expected at least one mounted filesystem");
}

fn sample_entry(name: &str, kind: FileKind, size: u64) -> FileEntry {
    FileEntry {
        name: name.to_string(),
        path: PathBuf::from(format!("/tmp/{name}")),
        kind,
        size,
        modified: None,
        is_hidden: name.starts_with('.'),
        is_symlink: false,
        readonly: false,
    }
}
