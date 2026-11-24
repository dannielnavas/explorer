use std::path::{Path, PathBuf};
use std::rc::Rc;

use relm4::gtk::gdk;
use relm4::gtk::glib;
use relm4::gtk::prelude::*;
use relm4::prelude::*;

/// Widget de cabecera de directorio con Breadcrumbs clicables y modo edición de ruta (`Ctrl+L`).
#[derive(Clone)]
pub struct BreadcrumbsBar {
    root: gtk::Stack,
    crumbs_box: gtk::Box,
    path_entry: gtk::Entry,
    on_navigate: Rc<dyn Fn(PathBuf)>,
}

impl BreadcrumbsBar {
    pub fn new<F>(initial_path: &Path, on_navigate: F) -> Self
    where
        F: Fn(PathBuf) + 'static,
    {
        let on_navigate: Rc<dyn Fn(PathBuf)> = Rc::new(on_navigate);

        let root = gtk::Stack::new();
        root.set_transition_type(gtk::StackTransitionType::Crossfade);
        root.set_transition_duration(120);
        root.set_hexpand(true);
        root.set_halign(gtk::Align::Fill);

        let crumbs_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        crumbs_box.set_valign(gtk::Align::Center);
        crumbs_box.add_css_class("breadcrumbs-box");

        let path_entry = gtk::Entry::new();
        path_entry.add_css_class("path-entry");
        path_entry.set_hexpand(true);
        path_entry.set_placeholder_text(Some("/ruta/al/directorio"));

        root.add_named(&crumbs_box, Some("crumbs"));
        root.add_named(&path_entry, Some("edit"));
        root.set_visible_child_name("crumbs");

        // Confirmar ruta al presionar Enter
        let nav_activate = Rc::clone(&on_navigate);
        let root_activate = root.clone();
        path_entry.connect_activate(move |entry| {
            let raw = entry.text();
            let expanded = expand_tilde(raw.trim());
            root_activate.set_visible_child_name("crumbs");
            if expanded.is_dir() {
                nav_activate(expanded);
            }
        });

        // Cancelar edición con Escape
        let key_ctrl = gtk::EventControllerKey::new();
        let root_esc = root.clone();
        key_ctrl.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape {
                root_esc.set_visible_child_name("crumbs");
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        path_entry.add_controller(key_ctrl);

        let bar = Self {
            root,
            crumbs_box,
            path_entry,
            on_navigate,
        };
        bar.set_path(initial_path);
        bar
    }

    pub fn widget(&self) -> &gtk::Stack {
        &self.root
    }

    /// Cambia a modo edición de ruta (invocado por `Ctrl+L` o el botón chevron).
    pub fn start_editing(&self, current_path: &Path) {
        self.path_entry
            .set_text(&current_path.to_string_lossy());
        self.root.set_visible_child_name("edit");
        self.path_entry.grab_focus();
        self.path_entry.select_region(0, -1);
    }

    /// Reconstruye los segmentos clicables de la ruta actual.
    pub fn set_path(&self, path: &Path) {
        while let Some(child) = self.crumbs_box.first_child() {
            self.crumbs_box.remove(&child);
        }

        let mut ancestors: Vec<PathBuf> = path.ancestors().map(Path::to_path_buf).collect();
        ancestors.reverse();

        // Mostrar los últimos 4 segmentos para mantener una cabecera limpia estilo GNOME
        let start = ancestors.len().saturating_sub(4);
        let visible = &ancestors[start..];

        for (idx, segment_path) in visible.iter().enumerate() {
            let is_last = idx + 1 == visible.len();
            let label_text = segment_path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "/".to_string());

            let btn = gtk::Button::with_label(&label_text);
            btn.add_css_class("breadcrumb-btn");
            if is_last {
                btn.add_css_class("directory-title");
            } else {
                btn.add_css_class("breadcrumb-parent");
            }

            let target = segment_path.clone();
            let nav = Rc::clone(&self.on_navigate);
            btn.connect_clicked(move |_| {
                nav(target.clone());
            });
            self.crumbs_box.append(&btn);

            if !is_last {
                let sep = gtk::Image::from_icon_name("go-next-symbolic");
                sep.add_css_class("breadcrumb-sep");
                sep.set_pixel_size(12);
                self.crumbs_box.append(&sep);
            }
        }

        // Botón chevron para editar la ruta directamente (como "Documents ⌄" en la referencia)
        let edit_btn = gtk::Button::from_icon_name("pan-down-symbolic");
        edit_btn.add_css_class("breadcrumb-edit-btn");
        edit_btn.set_tooltip_text(Some("Editar ruta (Ctrl+L)"));
        let this = self.clone();
        let current = path.to_path_buf();
        edit_btn.connect_clicked(move |_| {
            this.start_editing(&current);
        });
        self.crumbs_box.append(&edit_btn);
    }
}

fn expand_tilde(raw: &str) -> PathBuf {
    if let Some(rest) = raw.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    } else if raw == "~" {
        if let Some(home) = dirs::home_dir() {
            return home;
        }
    }
    PathBuf::from(raw)
}
