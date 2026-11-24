use std::f64::consts::PI;
use std::path::PathBuf;
use std::rc::Rc;

use relm4::gtk::prelude::*;
use relm4::prelude::*;

use explorer::fs::volumes::VolumeInfo;

/// Construye la fila inferior de tarjetas de almacenamiento con anillos circulares en Cairo.
pub fn build_disk_cards_strip<F>(volumes: &[VolumeInfo], on_select: F) -> gtk::ScrolledWindow
where
    F: Fn(PathBuf) + 'static,
{
    let callback = Rc::new(on_select);
    let container = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    container.add_css_class("disk-cards-strip");

    for volume in volumes {
        let card = build_disk_card(volume, Rc::clone(&callback));
        container.append(&card);
    }

    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .child(&container)
        .build();
    scroller.add_css_class("disk-cards-scroller");
    scroller
}

fn build_disk_card(volume: &VolumeInfo, on_select: Rc<dyn Fn(PathBuf)>) -> gtk::Button {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    row.set_valign(gtk::Align::Center);

    let icon_name = if volume.is_removable {
        "drive-removable-media-symbolic"
    } else {
        "drive-harddisk-symbolic"
    };
    let drive_icon = gtk::Image::from_icon_name(icon_name);
    drive_icon.set_pixel_size(26);
    drive_icon.add_css_class("disk-card-icon");
    if volume.mount_point == std::path::Path::new("/") {
        drive_icon.add_css_class("disk-card-icon-primary");
    }

    let text_col = gtk::Box::new(gtk::Orientation::Vertical, 3);
    text_col.set_valign(gtk::Align::Center);
    text_col.set_hexpand(true);

    let title = gtk::Label::new(Some(&volume.label));
    title.set_halign(gtk::Align::Start);
    title.add_css_class("disk-card-title");

    let subtitle = gtk::Label::new(Some(&volume.free_summary()));
    subtitle.set_halign(gtk::Align::Start);
    subtitle.add_css_class("disk-card-subtitle");

    text_col.append(&title);
    text_col.append(&subtitle);

    let ring = build_usage_ring(volume.used_fraction());

    row.append(&drive_icon);
    row.append(&text_col);
    row.append(&ring);

    let btn = gtk::Button::new();
    btn.set_child(Some(&row));
    btn.add_css_class("disk-card");
    btn.set_tooltip_text(Some(&format!(
        "Abrir {} ({})",
        volume.mount_point.display(),
        volume.file_system
    )));

    let mount_path = volume.mount_point.clone();
    btn.connect_clicked(move |_| {
        on_select(mount_path.clone());
    });

    btn
}

/// Dibuja un anillo de progreso circular con Cairo replicando el estilo de referencia.
fn build_usage_ring(used_fraction: f64) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.set_content_width(44);
    area.set_content_height(44);
    area.set_valign(gtk::Align::Center);

    let fraction = used_fraction.clamp(0.0, 1.0);
    area.set_draw_func(move |_area, cr, width, height| {
        let cx = f64::from(width) / 2.0;
        let cy = f64::from(height) / 2.0;
        let radius = (cx.min(cy) - 4.5).max(4.0);
        let line_width = 4.5;

        cr.set_line_width(line_width);
        cr.set_line_cap(gtk::cairo::LineCap::Round);

        // Pista de fondo (#1e293b)
        cr.set_source_rgba(0.118, 0.161, 0.231, 1.0);
        cr.arc(cx, cy, radius, 0.0, 2.0 * PI);
        let _ = cr.stroke();

        // Arco de uso (ámbar brillante como en la referencia; rojo si > 90%)
        if fraction > 0.005 {
            if fraction >= 0.90 {
                cr.set_source_rgba(0.937, 0.267, 0.267, 1.0);
            } else {
                cr.set_source_rgba(0.961, 0.620, 0.043, 1.0);
            }
            let start = -PI / 2.0;
            let end = start + fraction * 2.0 * PI;
            cr.arc(cx, cy, radius, start, end);
            let _ = cr.stroke();
        }
    });

    area
}
