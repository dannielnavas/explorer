use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use relm4::gtk::gio;
use relm4::gtk::glib;
use relm4::gtk::prelude::*;
use relm4::prelude::*;
use tokio::sync::mpsc;

use explorer::core::entry::{sort_entries, FileEntry, SortKey, SortOrder, ViewMode};
use explorer::core::history::NavigationHistory;
use explorer::core::search::SearchEngine;
use explorer::core::watcher::FileSystemWatcher;
use explorer::fs::reader;

use crate::app::{AppModel, AppMsg};
use crate::ui::breadcrumbs::BreadcrumbsBar;
use crate::ui::grid::{
    build_grid_factory, build_list_factory, open_file_externally, FileEntryObject,
};

/// Controlador de estado y vista para una pestaña individual del explorador.
#[derive(Clone)]
pub struct TabHandle {
    inner: Rc<TabInner>,
}

struct TabInner {
    root: gtk::Box,
    history: RefCell<NavigationHistory>,
    raw_entries: RefCell<Vec<FileEntry>>,
    search_engine: RefCell<SearchEngine>,
    search_query: RefCell<String>,
    store: gio::ListStore,
    selection: gtk::MultiSelection,
    content_stack: gtk::Stack,
    error_page: adw::StatusPage,
    breadcrumbs: BreadcrumbsBar,
    search_entry: gtk::Entry,
    status_label: gtk::Label,
    icon_size: Rc<Cell<i32>>,
    view_mode: Cell<ViewMode>,
    sort_key: Cell<SortKey>,
    sort_order: Cell<SortOrder>,
    show_hidden: Cell<bool>,
    load_generation: Cell<u64>,
    watcher: RefCell<Option<FileSystemWatcher>>,
    page: RefCell<Option<adw::TabPage>>,
    sender: ComponentSender<AppModel>,
}

impl TabHandle {
    pub fn new(
        initial_path: PathBuf,
        icon_size: f64,
        view_mode: ViewMode,
        sort_key: SortKey,
        sort_order: SortOrder,
        show_hidden: bool,
        sender: ComponentSender<AppModel>,
    ) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("tab-content");
        root.set_vexpand(true);
        root.set_hexpand(true);

        let icon_px = Rc::new(Cell::new(icon_size.round() as i32));
        let store = gio::ListStore::new::<FileEntryObject>();
        let selection = gtk::MultiSelection::new(Some(store.clone()));

        // Cabecera de directorio: Breadcrumbs + Búsqueda en vivo
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        header.add_css_class("directory-header");

        let sender_nav = sender.clone();
        let breadcrumbs = BreadcrumbsBar::new(&initial_path, move |target| {
            sender_nav.input(AppMsg::NavigateTo(target));
        });

        let search_entry = gtk::Entry::new();
        search_entry.set_placeholder_text(Some("Search"));
        search_entry.set_icon_from_icon_name(
            gtk::EntryIconPosition::Primary,
            Some("system-search-symbolic"),
        );
        search_entry.add_css_class("search-input");

        header.append(breadcrumbs.widget());
        header.append(&search_entry);
        root.append(&header);

        // Stack con transición Crossfade (loading / empty / error / grid / list)
        let content_stack = gtk::Stack::new();
        content_stack.set_transition_type(gtk::StackTransitionType::Crossfade);
        content_stack.set_transition_duration(120);
        content_stack.set_vexpand(true);
        content_stack.set_hexpand(true);

        // Estado: Cargando
        let loading_box = gtk::Box::new(gtk::Orientation::Vertical, 12);
        loading_box.set_valign(gtk::Align::Center);
        loading_box.set_halign(gtk::Align::Center);
        let spinner = gtk::Spinner::new();
        spinner.set_size_request(36, 36);
        spinner.start();
        let loading_label = gtk::Label::new(Some("Cargando directorio…"));
        loading_label.add_css_class("dim-label");
        loading_box.append(&spinner);
        loading_box.append(&loading_label);
        content_stack.add_named(&loading_box, Some("loading"));

        // Estado: Vacío (AdwStatusPage según GNOME HIG)
        let empty_page = adw::StatusPage::builder()
            .icon_name("folder-symbolic")
            .title("Carpeta vacía")
            .description("No se encontraron elementos en esta ubicación.")
            .build();
        content_stack.add_named(&empty_page, Some("empty"));

        // Estado: Error
        let error_page = adw::StatusPage::builder()
            .icon_name("dialog-warning-symbolic")
            .title("No se pudo abrir la carpeta")
            .build();
        content_stack.add_named(&error_page, Some("error"));

        // Vistas virtualizadas (GridView + ListView)
        let sender_mid_grid = sender.clone();
        let on_middle_click: Rc<dyn Fn(PathBuf)> = Rc::new(move |path| {
            sender_mid_grid.input(AppMsg::NewTabAt(path));
        });

        let grid_factory = build_grid_factory(Rc::clone(&icon_px), Rc::clone(&on_middle_click));
        let grid_view = gtk::GridView::new(Some(selection.clone()), Some(grid_factory));
        grid_view.add_css_class("file-grid");
        grid_view.set_min_columns(2);
        grid_view.set_max_columns(12);
        grid_view.set_single_click_activate(false);

        let grid_scroll = gtk::ScrolledWindow::builder()
            .vexpand(true)
            .hexpand(true)
            .child(&grid_view)
            .build();
        content_stack.add_named(&grid_scroll, Some("grid"));

        let list_factory = build_list_factory(on_middle_click);
        let list_view = gtk::ListView::new(Some(selection.clone()), Some(list_factory));
        list_view.add_css_class("file-list");
        list_view.set_single_click_activate(false);

        let list_scroll = gtk::ScrolledWindow::builder()
            .vexpand(true)
            .hexpand(true)
            .child(&list_view)
            .build();
        content_stack.add_named(&list_scroll, Some("list"));

        root.append(&content_stack);

        // Barra de estado inferior: contador + slider de zoom
        let status_bar = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        status_bar.add_css_class("status-bar");

        let status_label = gtk::Label::new(Some("0 items"));
        status_label.add_css_class("status-label");
        status_label.set_halign(gtk::Align::Start);
        status_label.set_hexpand(true);

        let zoom_slider = gtk::Scale::with_range(gtk::Orientation::Horizontal, 64.0, 256.0, 16.0);
        zoom_slider.set_value(icon_size);
        zoom_slider.set_draw_value(false);
        zoom_slider.set_width_request(130);
        zoom_slider.set_tooltip_text(Some("Tamaño de iconos"));
        zoom_slider.add_css_class("zoom-slider");

        status_bar.append(&status_label);
        status_bar.append(&zoom_slider);
        root.append(&status_bar);

        // Canal del observador de archivos (`notify`)
        let (watch_tx, mut watch_rx) = mpsc::unbounded_channel::<PathBuf>();
        let watcher = FileSystemWatcher::new(watch_tx).ok();

        let handle = Self {
            inner: Rc::new(TabInner {
                root,
                history: RefCell::new(NavigationHistory::new(initial_path.clone())),
                raw_entries: RefCell::new(Vec::new()),
                search_engine: RefCell::new(SearchEngine::new()),
                search_query: RefCell::new(String::new()),
                store,
                selection,
                content_stack,
                error_page,
                breadcrumbs,
                search_entry,
                status_label,
                icon_size: icon_px,
                view_mode: Cell::new(view_mode),
                sort_key: Cell::new(sort_key),
                sort_order: Cell::new(sort_order),
                show_hidden: Cell::new(show_hidden),
                load_generation: Cell::new(0),
                watcher: RefCell::new(watcher),
                page: RefCell::new(None),
                sender,
            }),
        };

        // Activar elemento con doble clic o Enter (en Grid y List)
        let h_grid = handle.clone();
        grid_view.connect_activate(move |_, pos| {
            h_grid.activate_position(pos);
        });
        let h_list = handle.clone();
        list_view.connect_activate(move |_, pos| {
            h_list.activate_position(pos);
        });

        // Actualizar barra de estado al cambiar selección
        let h_sel = handle.clone();
        handle
            .inner
            .selection
            .connect_selection_changed(move |_, _, _| {
                h_sel.update_status_label();
            });

        // Búsqueda difusa instantánea en vivo (`nucleo-matcher`)
        let h_search = handle.clone();
        handle.inner.search_entry.connect_changed(move |entry| {
            *h_search.inner.search_query.borrow_mut() = entry.text().to_string();
            h_search.rebuild_visible_list();
        });

        // Slider de zoom en tiempo real
        let h_zoom = handle.clone();
        zoom_slider.connect_value_changed(move |scale| {
            let new_px = scale.value().round() as i32;
            h_zoom.set_icon_size(new_px);
            h_zoom.inner.sender.input(AppMsg::IconSizeChanged(scale.value()));
        });

        // Auto-recarga cuando `notify` detecta cambios externos en disco
        let weak_inner = Rc::downgrade(&handle.inner);
        glib::MainContext::default().spawn_local(async move {
            while watch_rx.recv().await.is_some() {
                // Coalescer eventos en ráfaga
                while watch_rx.try_recv().is_ok() {}
                let Some(inner) = weak_inner.upgrade() else {
                    break;
                };
                let tab = TabHandle { inner };
                tab.reload();
            }
        });

        handle.reload();
        handle
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.inner.root
    }

    pub fn bind_page(&self, page: adw::TabPage) {
        self.inner.page.replace(Some(page));
        self.sync_page_metadata();
    }

    pub fn current_path(&self) -> PathBuf {
        self.inner.history.borrow().current().to_path_buf()
    }

    pub fn can_go_back(&self) -> bool {
        self.inner.history.borrow().can_go_back()
    }

    pub fn can_go_forward(&self) -> bool {
        self.inner.history.borrow().can_go_forward()
    }

    pub fn navigate_to(&self, path: PathBuf) {
        if !path.is_dir() {
            return;
        }
        self.inner.history.borrow_mut().push(path);
        self.inner.search_entry.set_text("");
        self.reload();
    }

    pub fn go_back(&self) -> bool {
        let moved = self.inner.history.borrow_mut().go_back().is_some();
        if moved {
            self.inner.search_entry.set_text("");
            self.reload();
        }
        moved
    }

    pub fn go_forward(&self) -> bool {
        let moved = self.inner.history.borrow_mut().go_forward().is_some();
        if moved {
            self.inner.search_entry.set_text("");
            self.reload();
        }
        moved
    }

    pub fn set_sort_key(&self, key: SortKey) {
        self.inner.sort_key.set(key);
        self.rebuild_visible_list();
    }

    pub fn toggle_sort_order(&self) {
        self.inner.sort_order.set(self.inner.sort_order.get().toggle());
        self.rebuild_visible_list();
    }

    pub fn set_view_mode(&self, mode: ViewMode) {
        self.inner.view_mode.set(mode);
        self.sync_visible_stack_page();
    }

    pub fn set_show_hidden(&self, show: bool) {
        if self.inner.show_hidden.get() != show {
            self.inner.show_hidden.set(show);
            self.reload();
        }
    }

    pub fn set_icon_size(&self, px: i32) {
        if self.inner.icon_size.get() != px {
            self.inner.icon_size.set(px);
            // Re-vincular filas visibles con el nuevo tamaño
            self.rebuild_visible_list();
        }
    }

    pub fn focus_search(&self) {
        self.inner.search_entry.grab_focus();
        self.inner.search_entry.select_region(0, -1);
    }

    pub fn start_editing_path(&self) {
        let path = self.current_path();
        self.inner.breadcrumbs.start_editing(&path);
    }

    /// Devuelve todos los `FileEntry` actualmente seleccionados en la vista.
    pub fn selected_entries(&self) -> Vec<FileEntry> {
        let bitset = self.inner.selection.selection();
        let mut selected = Vec::new();
        if let Some((iter, mut pos)) = gtk::BitsetIter::init_first(&bitset) {
            loop {
                if let Some(obj) = self
                    .inner
                    .store
                    .item(pos)
                    .and_then(|o| o.downcast::<FileEntryObject>().ok())
                {
                    if let Some(entry) = obj.entry() {
                        selected.push(entry);
                    }
                }
                match iter.next() {
                    Some(next_pos) => pos = next_pos,
                    None => break,
                }
            }
        }
        selected
    }

    /// Recarga el directorio actual en streaming sin bloquear el hilo de UI.
    pub fn reload(&self) {
        let path = self.current_path();
        let show_hidden = self.inner.show_hidden.get();

        self.inner.breadcrumbs.set_path(&path);
        self.sync_page_metadata();

        if let Some(watcher) = self.inner.watcher.borrow_mut().as_mut() {
            let _ = watcher.watch(&path);
        }

        let gen = self.inner.load_generation.get().wrapping_add(1);
        self.inner.load_generation.set(gen);

        self.inner.raw_entries.borrow_mut().clear();
        self.inner.store.remove_all();
        self.inner.content_stack.set_visible_child_name("loading");
        self.inner.status_label.set_text("Cargando…");

        let mut rx = reader::stream_directory(path.clone(), show_hidden, 256);
        let weak_inner = Rc::downgrade(&self.inner);

        glib::MainContext::default().spawn_local(async move {
            let mut failed: Option<String> = None;

            while let Some(batch_res) = rx.recv().await {
                let Some(inner) = weak_inner.upgrade() else {
                    return;
                };
                if inner.load_generation.get() != gen {
                    return;
                }
                match batch_res {
                    Ok(batch) => {
                        inner.raw_entries.borrow_mut().extend(batch);
                    }
                    Err(err) => {
                        failed = Some(err.to_string());
                        break;
                    }
                }
            }

            let Some(inner) = weak_inner.upgrade() else {
                return;
            };
            if inner.load_generation.get() != gen {
                return;
            }

            let tab = TabHandle { inner };
            if let Some(err_msg) = failed {
                tab.inner.error_page.set_description(Some(&err_msg));
                tab.inner.content_stack.set_visible_child_name("error");
                tab.inner.status_label.set_text("Error al leer directorio");
            } else {
                tab.rebuild_visible_list();
            }

            tab.inner.sender.input(AppMsg::ActiveTabStateChanged);
        });
    }

    fn activate_position(&self, pos: u32) {
        let Some(obj) = self
            .inner
            .store
            .item(pos)
            .and_then(|o| o.downcast::<FileEntryObject>().ok())
        else {
            return;
        };
        let Some(entry) = obj.entry() else {
            return;
        };

        if entry.is_dir() {
            self.navigate_to(entry.path);
        } else {
            open_file_externally(&entry.path);
        }
    }

    fn rebuild_visible_list(&self) {
        let mut raw = self.inner.raw_entries.borrow().clone();
        sort_entries(&mut raw, self.inner.sort_key.get(), self.inner.sort_order.get());

        let query = self.inner.search_query.borrow().clone();
        let filtered: Vec<FileEntry> = {
            let mut engine = self.inner.search_engine.borrow_mut();
            engine.filter(&raw, &query).into_iter().cloned().collect()
        };

        let objects: Vec<FileEntryObject> = filtered.into_iter().map(FileEntryObject::new).collect();
        let total = self.inner.store.n_items();
        self.inner.store.splice(0, total, &objects);

        self.sync_visible_stack_page();
        self.update_status_label();
    }

    fn sync_visible_stack_page(&self) {
        if self.inner.store.n_items() == 0 {
            self.inner.content_stack.set_visible_child_name("empty");
        } else {
            let page_name = match self.inner.view_mode.get() {
                ViewMode::Grid => "grid",
                ViewMode::List => "list",
            };
            self.inner.content_stack.set_visible_child_name(page_name);
        }
    }

    fn update_status_label(&self) {
        let total = self.inner.store.n_items();
        let selected = self.inner.selection.selection().size();
        let text = if selected > 0 {
            format!("{selected} selected • {total} items")
        } else if total == 1 {
            "1 item".to_string()
        } else {
            format!("{total} items")
        };
        self.inner.status_label.set_text(&text);
    }

    fn sync_page_metadata(&self) {
        let path = self.current_path();
        let title = tab_title_for_path(&path);
        if let Some(page) = self.inner.page.borrow().as_ref() {
            page.set_title(&title);
            page.set_tooltip(&path.to_string_lossy());
        }
    }
}

pub fn tab_title_for_path(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "/".to_string())
}
