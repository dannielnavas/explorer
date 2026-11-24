use std::cell::RefCell;

use relm4::gtk::gdk;
use relm4::gtk::glib;

use explorer::core::entry::FileKind;

const FOLDER_CYAN_SVG: &[u8] = include_bytes!("../../data/icons/folder-cyan.svg");
const FILE_PDF_SVG: &[u8] = include_bytes!("../../data/icons/file-pdf.svg");
const FILE_DOC_SVG: &[u8] = include_bytes!("../../data/icons/file-doc.svg");
const FILE_GENERIC_SVG: &[u8] = include_bytes!("../../data/icons/file-generic.svg");

struct IconCache {
    folder: Option<gdk::Texture>,
    pdf: Option<gdk::Texture>,
    doc: Option<gdk::Texture>,
    generic: Option<gdk::Texture>,
}

thread_local! {
    static CACHE: RefCell<Option<IconCache>> = const { RefCell::new(None) };
}

fn load_svg_texture(bytes: &'static [u8]) -> Option<gdk::Texture> {
    gdk::Texture::from_bytes(&glib::Bytes::from_static(bytes)).ok()
}

/// Devuelve la textura SVG en caché correspondiente al tipo de archivo.
pub fn texture_for_kind(kind: FileKind) -> Option<gdk::Texture> {
    CACHE.with(|cell| {
        let mut borrow = cell.borrow_mut();
        let cache = borrow.get_or_insert_with(|| IconCache {
            folder: load_svg_texture(FOLDER_CYAN_SVG),
            pdf: load_svg_texture(FILE_PDF_SVG),
            doc: load_svg_texture(FILE_DOC_SVG),
            generic: load_svg_texture(FILE_GENERIC_SVG),
        });

        match kind {
            FileKind::Directory => cache.folder.clone(),
            FileKind::Pdf => cache.pdf.clone(),
            FileKind::Document | FileKind::Code => cache.doc.clone(),
            _ => cache.generic.clone(),
        }
    })
}
