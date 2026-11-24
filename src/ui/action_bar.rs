use relm4::gtk::prelude::*;
use relm4::prelude::*;

use explorer::core::entry::{SortKey, ViewMode};

use crate::app::{AppModel, AppMsg};

/// Barra de herramientas de acciones (historial, portapapeles, CRUD, ordenación y vista).
#[derive(Clone)]
pub struct ActionBar {
    root: gtk::Box,
    back_btn: gtk::Button,
    forward_btn: gtk::Button,
    view_btn: gtk::Button,
    hidden_btn: gtk::ToggleButton,
}

impl ActionBar {
    pub fn new(
        initial_view: ViewMode,
        initial_hidden: bool,
        sender: ComponentSender<AppModel>,
    ) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        root.add_css_class("action-bar");

        // Historial: Atrás / Adelante
        let back_btn = make_action_button("go-previous-symbolic", "Atrás (Alt+←)");
        back_btn.set_sensitive(false);
        {
            let s = sender.clone();
            back_btn.connect_clicked(move |_| s.input(AppMsg::GoBack));
        }

        let forward_btn = make_action_button("go-next-symbolic", "Adelante (Alt+→)");
        forward_btn.set_sensitive(false);
        {
            let s = sender.clone();
            forward_btn.connect_clicked(move |_| s.input(AppMsg::GoForward));
        }

        root.append(&back_btn);
        root.append(&forward_btn);
        append_separator(&root);

        // Portapapeles: Cortar / Copiar / Pegar
        let cut_btn = make_action_button("edit-cut-symbolic", "Cortar (Ctrl+X)");
        {
            let s = sender.clone();
            cut_btn.connect_clicked(move |_| s.input(AppMsg::CutSelected));
        }

        let copy_btn = make_action_button("edit-copy-symbolic", "Copiar (Ctrl+C)");
        {
            let s = sender.clone();
            copy_btn.connect_clicked(move |_| s.input(AppMsg::CopySelected));
        }

        let paste_btn = make_action_button("edit-paste-symbolic", "Pegar (Ctrl+V)");
        {
            let s = sender.clone();
            paste_btn.connect_clicked(move |_| s.input(AppMsg::PasteHere));
        }

        root.append(&cut_btn);
        root.append(&copy_btn);
        root.append(&paste_btn);
        append_separator(&root);

        // Acciones de elemento: Renombrar / Nueva Carpeta / Papelera
        let rename_btn = make_action_button("document-edit-symbolic", "Renombrar (F2)");
        {
            let s = sender.clone();
            rename_btn.connect_clicked(move |_| s.input(AppMsg::PromptRename));
        }

        let new_folder_btn = make_action_button("folder-new-symbolic", "Nueva carpeta (Ctrl+Shift+N)");
        {
            let s = sender.clone();
            new_folder_btn.connect_clicked(move |_| s.input(AppMsg::PromptNewFolder));
        }

        let trash_btn = make_action_button("user-trash-symbolic", "Mover a la papelera (Supr)");
        {
            let s = sender.clone();
            trash_btn.connect_clicked(move |_| s.input(AppMsg::TrashSelected));
        }

        root.append(&rename_btn);
        root.append(&new_folder_btn);
        root.append(&trash_btn);
        append_separator(&root);

        // Menú de ordenación
        let sort_btn = build_sort_menu(sender.clone());
        root.append(&sort_btn);

        // Selector de vista (Cuadrícula / Lista)
        let view_icon = match initial_view {
            ViewMode::Grid => "view-list-symbolic",
            ViewMode::List => "view-grid-symbolic",
        };
        let view_btn = make_action_button(view_icon, "Cambiar vista (Cuadrícula / Lista)");
        {
            let s = sender.clone();
            view_btn.connect_clicked(move |_| s.input(AppMsg::ToggleViewMode));
        }
        root.append(&view_btn);

        // Mostrar archivos ocultos
        let hidden_btn = gtk::ToggleButton::new();
        hidden_btn.set_icon_name("view-reveal-symbolic");
        hidden_btn.set_tooltip_text(Some("Mostrar archivos ocultos (Ctrl+H)"));
        hidden_btn.add_css_class("action-btn");
        hidden_btn.set_active(initial_hidden);
        {
            let s = sender.clone();
            hidden_btn.connect_toggled(move |btn| {
                s.input(AppMsg::SetShowHidden(btn.is_active()));
            });
        }
        root.append(&hidden_btn);

        // Recargar directorio actual
        let refresh_btn = make_action_button("view-refresh-symbolic", "Actualizar (F5)");
        {
            let s = sender.clone();
            refresh_btn.connect_clicked(move |_| s.input(AppMsg::RefreshActiveTab));
        }
        root.append(&refresh_btn);

        Self {
            root,
            back_btn,
            forward_btn,
            view_btn,
            hidden_btn,
        }
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }

    pub fn set_history_state(&self, can_back: bool, can_forward: bool) {
        self.back_btn.set_sensitive(can_back);
        self.forward_btn.set_sensitive(can_forward);
    }

    pub fn set_view_mode(&self, mode: ViewMode) {
        let icon = match mode {
            ViewMode::Grid => "view-list-symbolic",
            ViewMode::List => "view-grid-symbolic",
        };
        self.view_btn.set_icon_name(icon);
    }

    pub fn set_show_hidden(&self, show: bool) {
        if self.hidden_btn.is_active() != show {
            self.hidden_btn.set_active(show);
        }
    }
}

fn make_action_button(icon_name: &str, tooltip: &str) -> gtk::Button {
    let btn = gtk::Button::from_icon_name(icon_name);
    btn.set_tooltip_text(Some(tooltip));
    btn.add_css_class("action-btn");
    btn
}

fn append_separator(bar: &gtk::Box) {
    let sep = gtk::Separator::new(gtk::Orientation::Vertical);
    sep.add_css_class("bar-separator");
    bar.append(&sep);
}

fn build_sort_menu(sender: ComponentSender<AppModel>) -> gtk::MenuButton {
    let popover_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
    popover_box.add_css_class("sort-popover-box");

    let popover = gtk::Popover::new();
    popover.set_child(Some(&popover_box));

    let options = [
        ("Nombre", SortKey::Name),
        ("Fecha de modificación", SortKey::Modified),
        ("Tamaño", SortKey::Size),
        ("Tipo", SortKey::Kind),
    ];

    for (label, key) in options {
        let btn = gtk::Button::with_label(label);
        btn.add_css_class("flat");
        btn.set_halign(gtk::Align::Fill);
        let s = sender.clone();
        let pop = popover.clone();
        btn.connect_clicked(move |_| {
            s.input(AppMsg::SetSortKey(key));
            pop.popdown();
        });
        popover_box.append(&btn);
    }

    popover_box.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

    let order_btn = gtk::Button::with_label("Invertir orden (Asc / Desc)");
    order_btn.add_css_class("flat");
    let s = sender.clone();
    let pop = popover.clone();
    order_btn.connect_clicked(move |_| {
        s.input(AppMsg::ToggleSortOrder);
        pop.popdown();
    });
    popover_box.append(&order_btn);

    let menu_btn = gtk::MenuButton::new();
    menu_btn.set_icon_name("view-sort-ascending-symbolic");
    menu_btn.set_tooltip_text(Some("Ordenar archivos"));
    menu_btn.add_css_class("action-btn");
    menu_btn.set_popover(Some(&popover));
    menu_btn
}
