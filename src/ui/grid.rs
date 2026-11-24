use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use relm4::gtk::gdk;
use relm4::gtk::gio;
use relm4::gtk::glib;
use relm4::gtk::prelude::*;
use relm4::gtk::subclass::prelude::*;
use relm4::prelude::*;

use explorer::core::entry::{FileEntry, FileKind};
use explorer::fs::thumbnails;

use crate::ui::icons;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct FileEntryObject {
        pub entry: RefCell<Option<FileEntry>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for FileEntryObject {
        const NAME: &'static str = "ExplorerFileEntryObject";
        type Type = super::FileEntryObject;
        type ParentType = glib::Object;
    }

    impl ObjectImpl for FileEntryObject {}
}

glib::wrapper! {
    /// Wrapper `GObject` ligero para almacenar `FileEntry` en un `gio::ListStore` virtualizado.
    pub struct FileEntryObject(ObjectSubclass<imp::FileEntryObject>);
}

impl FileEntryObject {
    pub fn new(entry: FileEntry) -> Self {
        let obj: Self = glib::Object::new();
        obj.imp().entry.replace(Some(entry));
        obj
    }

    pub fn entry(&self) -> Option<FileEntry> {
        self.imp().entry.borrow().clone()
    }
}

/// Construye la fábrica virtualizada (`SignalListItemFactory`) para `gtk::GridView`.
pub fn build_grid_factory(
    icon_size: Rc<Cell<i32>>,
    on_middle_click: Rc<dyn Fn(PathBuf)>,
) -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();

    factory.connect_setup(move |_, list_item| {
        let Some(item) = list_item.downcast_ref::<gtk::ListItem>() else {
            return;
        };

        let tile = gtk::Box::new(gtk::Orientation::Vertical, 6);
        tile.add_css_class("file-tile");
        tile.set_halign(gtk::Align::Center);
        tile.set_valign(gtk::Align::Start);

        let picture = gtk::Picture::new();
        picture.set_can_shrink(true);
        picture.set_content_fit(gtk::ContentFit::Contain);
        picture.set_halign(gtk::Align::Center);
        picture.add_css_class("file-icon");

        let label = gtk::Label::new(None);
        label.set_wrap(true);
        label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        label.set_lines(2);
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        label.set_justify(gtk::Justification::Center);
        label.set_max_width_chars(14);
        label.add_css_class("file-label");

        tile.append(&picture);
        tile.append(&label);
        item.set_child(Some(&tile));
    });

    let size_for_bind = Rc::clone(&icon_size);
    factory.connect_bind(move |_, list_item| {
        let Some(item) = list_item.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let Some(obj) = item
            .item()
            .and_then(|o| o.downcast::<FileEntryObject>().ok())
        else {
            return;
        };
        let Some(entry) = obj.entry() else {
            return;
        };
        let Some(tile) = item.child().and_then(|c| c.downcast::<gtk::Box>().ok()) else {
            return;
        };

        let px = size_for_bind.get();
        let tile_width = (px + 34).max(96);
        tile.set_width_request(tile_width);

        if let Some(picture) = tile
            .first_child()
            .and_then(|w| w.downcast::<gtk::Picture>().ok())
        {
            picture.set_size_request(px, px);
            bind_entry_texture(&picture, &entry);
        }

        if let Some(label) = tile
            .last_child()
            .and_then(|w| w.downcast::<gtk::Label>().ok())
        {
            label.set_text(&entry.name);
        }

        tile.set_tooltip_text(Some(&format!(
            "{}\n{} • {}",
            entry.name,
            entry.formatted_size(),
            entry.formatted_modified()
        )));

        // Clic central en carpetas para abrir en una nueva pestaña
        if entry.is_dir() {
            let gesture = gtk::GestureClick::new();
            gesture.set_button(2);
            let path = entry.path.clone();
            let cb = Rc::clone(&on_middle_click);
            gesture.connect_released(move |_, _, _, _| {
                cb(path.clone());
            });
            tile.add_controller(gesture);
        }
    });

    factory
}

/// Construye la fábrica virtualizada (`SignalListItemFactory`) para `gtk::ListView` (modo lista).
pub fn build_list_factory(on_middle_click: Rc<dyn Fn(PathBuf)>) -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();

    factory.connect_setup(move |_, list_item| {
        let Some(item) = list_item.downcast_ref::<gtk::ListItem>() else {
            return;
        };

        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        row.add_css_class("file-list-row");
        row.set_valign(gtk::Align::Center);

        let picture = gtk::Picture::new();
        picture.set_size_request(24, 24);
        picture.set_content_fit(gtk::ContentFit::Contain);

        let name_label = gtk::Label::new(None);
        name_label.set_halign(gtk::Align::Start);
        name_label.set_hexpand(true);
        name_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        name_label.add_css_class("file-list-name");

        let size_label = gtk::Label::new(None);
        size_label.set_halign(gtk::Align::End);
        size_label.set_width_chars(10);
        size_label.add_css_class("file-list-meta");

        let date_label = gtk::Label::new(None);
        date_label.set_halign(gtk::Align::End);
        date_label.set_width_chars(16);
        date_label.add_css_class("file-list-meta");

        row.append(&picture);
        row.append(&name_label);
        row.append(&size_label);
        row.append(&date_label);
        item.set_child(Some(&row));
    });

    factory.connect_bind(move |_, list_item| {
        let Some(item) = list_item.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let Some(obj) = item
            .item()
            .and_then(|o| o.downcast::<FileEntryObject>().ok())
        else {
            return;
        };
        let Some(entry) = obj.entry() else {
            return;
        };
        let Some(row) = item.child().and_then(|c| c.downcast::<gtk::Box>().ok()) else {
            return;
        };

        let mut child = row.first_child();
        if let Some(pic) = child
            .as_ref()
            .and_then(|w| w.downcast_ref::<gtk::Picture>())
        {
            bind_entry_texture(pic, &entry);
        }
        child = child.and_then(|w| w.next_sibling());
        if let Some(lbl) = child.as_ref().and_then(|w| w.downcast_ref::<gtk::Label>()) {
            lbl.set_text(&entry.name);
        }
        child = child.and_then(|w| w.next_sibling());
        if let Some(lbl) = child.as_ref().and_then(|w| w.downcast_ref::<gtk::Label>()) {
            lbl.set_text(&entry.formatted_size());
        }
        child = child.and_then(|w| w.next_sibling());
        if let Some(lbl) = child.as_ref().and_then(|w| w.downcast_ref::<gtk::Label>()) {
            lbl.set_text(&entry.formatted_modified());
        }

        if entry.is_dir() {
            let gesture = gtk::GestureClick::new();
            gesture.set_button(2);
            let path = entry.path.clone();
            let cb = Rc::clone(&on_middle_click);
            gesture.connect_released(move |_, _, _, _| {
                cb(path.clone());
            });
            row.add_controller(gesture);
        }
    });

    factory
}

/// Asigna la textura de icono y, para imágenes, carga o genera la miniatura XDG de forma asíncrona.
fn bind_entry_texture(picture: &gtk::Picture, entry: &FileEntry) {
    if let Some(texture) = icons::texture_for_kind(entry.kind) {
        picture.set_paintable(Some(&texture));
    }

    if entry.kind != FileKind::Image {
        return;
    }

    // Si ya existe en caché FreeDesktop, usarla al instante
    if let Some(cached) = thumbnails::existing_valid_thumbnail(&entry.path, entry.modified) {
        if let Ok(tex) = gdk::Texture::from_filename(&cached) {
            picture.set_paintable(Some(&tex));
            return;
        }
    }

    // Generar miniatura en segundo plano sin bloquear la UI
    let weak_pic = picture.downgrade();
    let file_path = entry.path.clone();
    glib::MainContext::default().spawn_local(async move {
        if let Ok(thumb_path) = thumbnails::generate_thumbnail(file_path).await {
            if let Some(pic) = weak_pic.upgrade() {
                if let Ok(tex) = gdk::Texture::from_filename(&thumb_path) {
                    pic.set_paintable(Some(&tex));
                }
            }
        }
    });
}

/// Abre un archivo con la aplicación predeterminada del sistema operativo.
pub fn open_file_externally(path: &std::path::Path) {
    let file = gio::File::for_path(path);
    let uri = file.uri();
    let _ = gio::AppInfo::launch_default_for_uri(&uri, None::<&gio::AppLaunchContext>);
}
