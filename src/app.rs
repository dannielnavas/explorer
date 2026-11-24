use std::path::PathBuf;

use relm4::adw::prelude::*;
use relm4::gtk::gdk;
use relm4::gtk::glib;
use relm4::gtk::prelude::*;
use relm4::prelude::*;

use explorer::config::AppConfig;
use explorer::core::entry::{SortKey, ViewMode};
use explorer::fs::operations::{
    self, ClipboardMode, ClipboardState, CollisionPolicy,
};
use explorer::fs::volumes;

use crate::ui::action_bar::ActionBar;
use crate::ui::disk_card::build_disk_cards_strip;
use crate::ui::quick_access::QuickAccessBar;
use crate::ui::tab::TabHandle;
use crate::ui::window::show_text_prompt_dialog;

pub struct AppModel {
    config: AppConfig,
    window: adw::ApplicationWindow,
    toast_overlay: adw::ToastOverlay,
    tab_view: adw::TabView,
    tabs: Vec<(adw::TabPage, TabHandle)>,
    quick_bar: QuickAccessBar,
    action_bar: ActionBar,
    clipboard: Option<ClipboardState>,
}

#[derive(Debug)]
pub enum AppMsg {
    NewTabAt(PathBuf),
    NewTabFromCurrent,
    CloseCurrentTab,
    NavigateTo(PathBuf),
    GoHome,
    GoBack,
    GoForward,
    RefreshActiveTab,
    FocusSearch,
    EditPath,
    CutSelected,
    CopySelected,
    PasteHere,
    PromptRename,
    PerformRename { path: PathBuf, new_name: String },
    PromptNewFolder,
    PerformNewFolder { parent: PathBuf, name: String },
    TrashSelected,
    SetSortKey(SortKey),
    ToggleSortOrder,
    ToggleViewMode,
    SetShowHidden(bool),
    ToggleHidden,
    IconSizeChanged(f64),
    ActiveTabStateChanged,
    ShowToast(String),
    ShowHighPriorityToast(String),
}

#[relm4::component(pub)]
impl SimpleComponent for AppModel {
    type Init = AppConfig;
    type Input = AppMsg;
    type Output = ();

    view! {
        #[root]
        adw::ApplicationWindow {
            set_title: Some("Explorer"),
            set_default_width: 1200,
            set_default_height: 780,
            set_width_request: 360,
            add_css_class: "explorer-window",

            #[local_ref]
            toast_overlay -> adw::ToastOverlay {
                #[wrap(Some)]
                set_child = &adw::ToolbarView {
                    add_top_bar = &adw::HeaderBar {
                        add_css_class: "explorer-headerbar",
                        set_show_start_title_buttons: false,
                        set_show_end_title_buttons: true,

                        pack_start = &gtk::Button {
                            set_icon_name: "go-home-symbolic",
                            set_tooltip_text: Some("Carpeta personal (Home)"),
                            add_css_class: "home-btn",
                            connect_clicked => AppMsg::GoHome,
                        },

                        #[wrap(Some)]
                        set_title_widget = &adw::TabBar {
                            add_css_class: "explorer-tabbar",
                            set_hexpand: true,
                            set_autohide: false,
                            set_expand_tabs: false,
                            set_view: Some(tab_view),

                            #[wrap(Some)]
                            set_end_action_widget = &gtk::Button {
                                set_icon_name: "tab-new-symbolic",
                                set_tooltip_text: Some("Nueva pestaña (Ctrl+T)"),
                                add_css_class: "new-tab-btn",
                                connect_clicked => AppMsg::NewTabFromCurrent,
                            },
                        },
                    },

                    #[wrap(Some)]
                    set_content = &gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_vexpand: true,

                        append: quick_bar.widget(),
                        append: action_bar.widget(),

                        #[local_ref]
                        tab_view -> adw::TabView {
                            set_vexpand: true,
                            set_hexpand: true,
                        },

                        append: &disk_strip,
                    },
                },
            },
        }
    }

    fn init(
        config: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        // Forzar tema oscuro profundo #0b0f19 conforme al diseño de referencia
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);

        let toast_overlay = adw::ToastOverlay::new();
        let tab_view = adw::TabView::new();

        // Mantener al menos una pestaña abierta
        tab_view.connect_close_page(|view, page| {
            let allow_close = view.n_pages() > 1;
            view.close_page_finish(page, allow_close);
            glib::Propagation::Stop
        });

        let sender_tab_switch = sender.clone();
        tab_view.connect_selected_page_notify(move |_| {
            sender_tab_switch.input(AppMsg::ActiveTabStateChanged);
        });

        let xdg_locations = volumes::xdg_quick_locations();
        let disk_volumes = volumes::list_volumes();

        let quick_bar = QuickAccessBar::new(
            &xdg_locations,
            &disk_volumes,
            &config.default_path,
            sender.clone(),
        );
        let action_bar = ActionBar::new(config.view_mode, config.show_hidden, sender.clone());

        let sender_disk = sender.clone();
        let disk_strip = build_disk_cards_strip(&disk_volumes, move |mount_path| {
            sender_disk.input(AppMsg::NavigateTo(mount_path));
        });

        // Atajos de teclado globales en la ventana principal
        install_keyboard_shortcuts(&root, sender.clone());

        let initial_path = config.default_path.clone();
        let mut model = AppModel {
            config,
            window: root.clone(),
            toast_overlay,
            tab_view,
            tabs: Vec::new(),
            quick_bar,
            action_bar,
            clipboard: None,
        };

        model.open_tab_at(initial_path, sender.clone());

        let toast_overlay = &model.toast_overlay;
        let tab_view = &model.tab_view;
        let quick_bar = &model.quick_bar;
        let action_bar = &model.action_bar;

        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        // Limpiar pestañas cerradas del registro interno
        self.prune_closed_tabs();

        match msg {
            AppMsg::NewTabAt(path) => {
                self.open_tab_at(path, sender);
            }
            AppMsg::NewTabFromCurrent => {
                let path = self
                    .active_tab()
                    .map(|t| t.current_path())
                    .unwrap_or_else(|| self.config.default_path.clone());
                self.open_tab_at(path, sender);
            }
            AppMsg::CloseCurrentTab => {
                if self.tab_view.n_pages() > 1 {
                    if let Some(page) = self.tab_view.selected_page() {
                        self.tab_view.close_page(&page);
                    }
                }
            }
            AppMsg::NavigateTo(path) => {
                if let Some(tab) = self.active_tab() {
                    tab.navigate_to(path);
                    self.sync_chrome_with_active_tab();
                }
            }
            AppMsg::GoHome => {
                let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
                if let Some(tab) = self.active_tab() {
                    tab.navigate_to(home);
                    self.sync_chrome_with_active_tab();
                }
            }
            AppMsg::GoBack => {
                if let Some(tab) = self.active_tab() {
                    if tab.go_back() {
                        self.sync_chrome_with_active_tab();
                    }
                }
            }
            AppMsg::GoForward => {
                if let Some(tab) = self.active_tab() {
                    if tab.go_forward() {
                        self.sync_chrome_with_active_tab();
                    }
                }
            }
            AppMsg::RefreshActiveTab => {
                if let Some(tab) = self.active_tab() {
                    tab.reload();
                }
            }
            AppMsg::FocusSearch => {
                if let Some(tab) = self.active_tab() {
                    tab.focus_search();
                }
            }
            AppMsg::EditPath => {
                if let Some(tab) = self.active_tab() {
                    tab.start_editing_path();
                }
            }
            AppMsg::CutSelected => {
                self.set_clipboard_from_selection(ClipboardMode::Cut);
            }
            AppMsg::CopySelected => {
                self.set_clipboard_from_selection(ClipboardMode::Copy);
            }
            AppMsg::PasteHere => {
                let Some(tab) = self.active_tab() else {
                    return;
                };
                let Some(clip) = self.clipboard.clone() else {
                    self.show_toast("El portapapeles está vacío", false);
                    return;
                };
                let dest_dir = tab.current_path();
                let is_cut = clip.mode == ClipboardMode::Cut;
                if is_cut {
                    self.clipboard = None;
                }
                let s = sender.clone();
                glib::MainContext::default().spawn_local(async move {
                    match operations::paste_entries(&clip, &dest_dir, CollisionPolicy::Rename).await {
                        Ok(created) => {
                            let count = created.len();
                            let verb = if is_cut { "Movidos" } else { "Copiados" };
                            s.input(AppMsg::ShowToast(format!("{verb}: {count} elemento(s)")));
                            s.input(AppMsg::RefreshActiveTab);
                        }
                        Err(err) => {
                            s.input(AppMsg::ShowHighPriorityToast(format!(
                                "Error al pegar: {err}"
                            )));
                        }
                    }
                });
            }
            AppMsg::PromptRename => {
                let Some(tab) = self.active_tab() else {
                    return;
                };
                let selected = tab.selected_entries();
                let Some(first) = selected.first() else {
                    self.show_toast("Selecciona un archivo o carpeta para renombrar", false);
                    return;
                };
                let target_path = first.path.clone();
                let current_name = first.name.clone();
                let s = sender.clone();
                show_text_prompt_dialog(
                    &self.window,
                    "Renombrar elemento",
                    "Introduce el nuevo nombre:",
                    &current_name,
                    "Renombrar",
                    move |new_name| {
                        s.input(AppMsg::PerformRename {
                            path: target_path.clone(),
                            new_name,
                        });
                    },
                );
            }
            AppMsg::PerformRename { path, new_name } => {
                let s = sender.clone();
                glib::MainContext::default().spawn_local(async move {
                    match operations::rename_entry(&path, &new_name).await {
                        Ok(_) => {
                            s.input(AppMsg::ShowToast(format!("Renombrado a «{new_name}»")));
                            s.input(AppMsg::RefreshActiveTab);
                        }
                        Err(err) => {
                            s.input(AppMsg::ShowHighPriorityToast(format!(
                                "No se pudo renombrar: {err}"
                            )));
                        }
                    }
                });
            }
            AppMsg::PromptNewFolder => {
                let Some(tab) = self.active_tab() else {
                    return;
                };
                let parent = tab.current_path();
                let s = sender.clone();
                show_text_prompt_dialog(
                    &self.window,
                    "Nueva carpeta",
                    "Nombre de la nueva carpeta:",
                    "Nueva carpeta",
                    "Crear",
                    move |name| {
                        s.input(AppMsg::PerformNewFolder {
                            parent: parent.clone(),
                            name,
                        });
                    },
                );
            }
            AppMsg::PerformNewFolder { parent, name } => {
                let s = sender.clone();
                glib::MainContext::default().spawn_local(async move {
                    match operations::create_folder(&parent, &name).await {
                        Ok(_) => {
                            s.input(AppMsg::ShowToast(format!("Carpeta «{name}» creada")));
                            s.input(AppMsg::RefreshActiveTab);
                        }
                        Err(err) => {
                            s.input(AppMsg::ShowHighPriorityToast(format!(
                                "No se pudo crear la carpeta: {err}"
                            )));
                        }
                    }
                });
            }
            AppMsg::TrashSelected => {
                let Some(tab) = self.active_tab() else {
                    return;
                };
                let selected = tab.selected_entries();
                if selected.is_empty() {
                    self.show_toast("Selecciona elementos para mover a la papelera", false);
                    return;
                }
                let count = selected.len();
                let paths: Vec<PathBuf> = selected.into_iter().map(|e| e.path).collect();
                let s = sender.clone();
                // Patrón GNOME HIG Tier 1: mover a papelera sin diálogo bloqueante + Toast HIGH
                glib::MainContext::default().spawn_local(async move {
                    match operations::move_to_trash(paths).await {
                        Ok(()) => {
                            let label = if count == 1 {
                                "1 elemento movido a la papelera".to_string()
                            } else {
                                format!("{count} elementos movidos a la papelera")
                            };
                            s.input(AppMsg::ShowHighPriorityToast(label));
                            s.input(AppMsg::RefreshActiveTab);
                        }
                        Err(err) => {
                            s.input(AppMsg::ShowHighPriorityToast(format!(
                                "Error al mover a la papelera: {err}"
                            )));
                        }
                    }
                });
            }
            AppMsg::SetSortKey(key) => {
                self.config.sort_key = key;
                self.config.save();
                for (_, tab) in &self.tabs {
                    tab.set_sort_key(key);
                }
            }
            AppMsg::ToggleSortOrder => {
                self.config.sort_order = self.config.sort_order.toggle();
                self.config.save();
                for (_, tab) in &self.tabs {
                    tab.toggle_sort_order();
                }
            }
            AppMsg::ToggleViewMode => {
                self.config.view_mode = match self.config.view_mode {
                    ViewMode::Grid => ViewMode::List,
                    ViewMode::List => ViewMode::Grid,
                };
                self.config.save();
                self.action_bar.set_view_mode(self.config.view_mode);
                for (_, tab) in &self.tabs {
                    tab.set_view_mode(self.config.view_mode);
                }
            }
            AppMsg::SetShowHidden(show) => {
                self.config.show_hidden = show;
                self.config.save();
                self.action_bar.set_show_hidden(show);
                for (_, tab) in &self.tabs {
                    tab.set_show_hidden(show);
                }
            }
            AppMsg::ToggleHidden => {
                let next = !self.config.show_hidden;
                self.config.show_hidden = next;
                self.config.save();
                self.action_bar.set_show_hidden(next);
                for (_, tab) in &self.tabs {
                    tab.set_show_hidden(next);
                }
            }
            AppMsg::IconSizeChanged(size) => {
                self.config.icon_size = size.clamp(64.0, 256.0);
                self.config.save();
            }
            AppMsg::ActiveTabStateChanged => {
                self.sync_chrome_with_active_tab();
            }
            AppMsg::ShowToast(message) => {
                self.show_toast(&message, false);
            }
            AppMsg::ShowHighPriorityToast(message) => {
                self.show_toast(&message, true);
            }
        }
    }
}

impl AppModel {
    fn open_tab_at(&mut self, path: PathBuf, sender: ComponentSender<Self>) {
        let tab = TabHandle::new(
            path,
            self.config.icon_size,
            self.config.view_mode,
            self.config.sort_key,
            self.config.sort_order,
            self.config.show_hidden,
            sender,
        );
        let page = self.tab_view.append(tab.widget());
        tab.bind_page(page.clone());
        self.tab_view.set_selected_page(&page);
        self.tabs.push((page, tab));
        self.sync_chrome_with_active_tab();
    }

    fn active_tab(&self) -> Option<TabHandle> {
        let selected = self.tab_view.selected_page()?;
        self.tabs
            .iter()
            .find(|(page, _)| page == &selected)
            .map(|(_, tab)| tab.clone())
    }

    fn prune_closed_tabs(&mut self) {
        self.tabs
            .retain(|(page, _)| self.tab_view.page_position(page) >= 0);
    }

    fn sync_chrome_with_active_tab(&self) {
        if let Some(tab) = self.active_tab() {
            let path = tab.current_path();
            self.quick_bar.set_active_path(&path);
            self.action_bar
                .set_history_state(tab.can_go_back(), tab.can_go_forward());
        }
    }

    fn set_clipboard_from_selection(&mut self, mode: ClipboardMode) {
        let Some(tab) = self.active_tab() else {
            return;
        };
        let selected = tab.selected_entries();
        if selected.is_empty() {
            self.show_toast("No hay elementos seleccionados", false);
            return;
        }
        let paths: Vec<PathBuf> = selected.into_iter().map(|e| e.path).collect();
        let count = paths.len();

        // Sincronizar con el portapapeles del sistema (MIME x-special/gnome-copied-files)
        let payload = operations::format_gnome_copied_files(mode, &paths);
        if let Some(display) = gdk::Display::default() {
            let provider = gdk::ContentProvider::for_bytes(
                "x-special/gnome-copied-files",
                &glib::Bytes::from(payload.as_bytes()),
            );
            let _ = display.clipboard().set_content(Some(&provider));
        }

        self.clipboard = Some(ClipboardState { mode, paths });
        let verb = match mode {
            ClipboardMode::Copy => "Copiado",
            ClipboardMode::Cut => "Cortado",
        };
        self.show_toast(&format!("{verb}: {count} elemento(s)"), false);
    }

    fn show_toast(&self, message: &str, high_priority: bool) {
        let toast = adw::Toast::builder()
            .title(message)
            .priority(if high_priority {
                adw::ToastPriority::High
            } else {
                adw::ToastPriority::Normal
            })
            .build();
        self.toast_overlay.add_toast(toast);
    }
}

fn install_keyboard_shortcuts(window: &adw::ApplicationWindow, sender: ComponentSender<AppModel>) {
    let key_ctrl = gtk::EventControllerKey::new();
    let win_weak = window.downgrade();

    key_ctrl.connect_key_pressed(move |_, key, _, state| {
        let ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
        let shift = state.contains(gdk::ModifierType::SHIFT_MASK);
        let alt = state.contains(gdk::ModifierType::ALT_MASK);

        if ctrl && !shift && !alt {
            match key {
                gdk::Key::t | gdk::Key::T => {
                    sender.input(AppMsg::NewTabFromCurrent);
                    return glib::Propagation::Stop;
                }
                gdk::Key::w | gdk::Key::W => {
                    sender.input(AppMsg::CloseCurrentTab);
                    return glib::Propagation::Stop;
                }
                gdk::Key::l | gdk::Key::L => {
                    sender.input(AppMsg::EditPath);
                    return glib::Propagation::Stop;
                }
                gdk::Key::f | gdk::Key::F => {
                    sender.input(AppMsg::FocusSearch);
                    return glib::Propagation::Stop;
                }
                gdk::Key::h | gdk::Key::H => {
                    sender.input(AppMsg::ToggleHidden);
                    return glib::Propagation::Stop;
                }
                gdk::Key::c | gdk::Key::C => {
                    sender.input(AppMsg::CopySelected);
                    return glib::Propagation::Stop;
                }
                gdk::Key::x | gdk::Key::X => {
                    sender.input(AppMsg::CutSelected);
                    return glib::Propagation::Stop;
                }
                gdk::Key::v | gdk::Key::V => {
                    sender.input(AppMsg::PasteHere);
                    return glib::Propagation::Stop;
                }
                gdk::Key::q | gdk::Key::Q => {
                    if let Some(win) = win_weak.upgrade() {
                        win.close();
                    }
                    return glib::Propagation::Stop;
                }
                _ => {}
            }
        } else if ctrl && shift && matches!(key, gdk::Key::N | gdk::Key::n) {
            sender.input(AppMsg::PromptNewFolder);
            return glib::Propagation::Stop;
        } else if alt && !ctrl {
            match key {
                gdk::Key::Left => {
                    sender.input(AppMsg::GoBack);
                    return glib::Propagation::Stop;
                }
                gdk::Key::Right => {
                    sender.input(AppMsg::GoForward);
                    return glib::Propagation::Stop;
                }
                _ => {}
            }
        } else if !ctrl && !alt {
            match key {
                gdk::Key::F2 => {
                    sender.input(AppMsg::PromptRename);
                    return glib::Propagation::Stop;
                }
                gdk::Key::F5 => {
                    sender.input(AppMsg::RefreshActiveTab);
                    return glib::Propagation::Stop;
                }
                gdk::Key::Delete => {
                    sender.input(AppMsg::TrashSelected);
                    return glib::Propagation::Stop;
                }
                _ => {}
            }
        }

        glib::Propagation::Proceed
    });

    window.add_controller(key_ctrl);
}
