use std::path::{Path, PathBuf};

use relm4::gtk::prelude::*;
use relm4::prelude::*;

use explorer::fs::volumes::{QuickLocation, VolumeInfo};

use crate::app::{AppModel, AppMsg};

/// Barra superior de acceso rápido (carpetas XDG) y volúmenes montados.
#[derive(Clone)]
pub struct QuickAccessBar {
    root: gtk::Box,
    pills: Vec<(PathBuf, gtk::Button)>,
}

impl QuickAccessBar {
    pub fn new(
        xdg_locations: &[QuickLocation],
        volumes: &[VolumeInfo],
        active_path: &Path,
        sender: ComponentSender<AppModel>,
    ) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        root.add_css_class("quick-bar");

        let mut pills = Vec::with_capacity(xdg_locations.len() + volumes.len());

        for loc in xdg_locations {
            let btn = gtk::Button::from_icon_name(loc.icon_name);
            btn.set_tooltip_text(Some(loc.label));
            btn.add_css_class("quick-pill");

            let sender_clone = sender.clone();
            let target = loc.path.clone();
            btn.connect_clicked(move |_| {
                sender_clone.input(AppMsg::NavigateTo(target.clone()));
            });

            root.append(&btn);
            pills.push((loc.path.clone(), btn));
        }

        let sep = gtk::Separator::new(gtk::Orientation::Vertical);
        sep.add_css_class("bar-separator");
        root.append(&sep);

        for vol in volumes {
            let pill_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            let icon_name = if vol.is_removable {
                "drive-removable-media-symbolic"
            } else {
                "drive-harddisk-symbolic"
            };
            let icon = gtk::Image::from_icon_name(icon_name);
            let text = gtk::Label::new(Some(&vol.label));
            pill_box.append(&icon);
            pill_box.append(&text);

            let btn = gtk::Button::new();
            btn.set_child(Some(&pill_box));
            btn.add_css_class("volume-pill");
            btn.set_tooltip_text(Some(&vol.free_summary()));

            let sender_clone = sender.clone();
            let target = vol.mount_point.clone();
            btn.connect_clicked(move |_| {
                sender_clone.input(AppMsg::NavigateTo(target.clone()));
            });

            root.append(&btn);
            pills.push((vol.mount_point.clone(), btn));
        }

        let sep_net = gtk::Separator::new(gtk::Orientation::Vertical);
        sep_net.add_css_class("bar-separator");
        root.append(&sep_net);

        let net_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        net_box.append(&gtk::Image::from_icon_name("network-workgroup-symbolic"));
        net_box.append(&gtk::Label::new(Some("Network")));
        let net_btn = gtk::Button::new();
        net_btn.set_child(Some(&net_box));
        net_btn.add_css_class("volume-pill");
        net_btn.set_tooltip_text(Some("Ubicaciones de red"));
        let sender_net = sender.clone();
        net_btn.connect_clicked(move |_| {
            sender_net.input(AppMsg::ShowToast(
                "Navegación de red GVFS disponible en Linux/GNOME".to_string(),
            ));
        });
        root.append(&net_btn);

        let bar = Self { root, pills };
        bar.set_active_path(active_path);
        bar
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }

    /// Resalta el acceso rápido o volumen que coincide con la ruta activa.
    pub fn set_active_path(&self, current: &Path) {
        for (path, btn) in &self.pills {
            if path == current {
                btn.add_css_class("active-pill");
            } else {
                btn.remove_css_class("active-pill");
            }
        }
    }
}
