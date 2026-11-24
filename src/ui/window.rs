use relm4::adw::prelude::*;
use relm4::gtk::prelude::*;
use relm4::prelude::*;

/// Muestra un diálogo nativo `AdwAlertDialog` con campo de texto para renombrar o crear carpeta.
pub fn show_text_prompt_dialog<F>(
    parent: &impl IsA<gtk::Widget>,
    heading: &str,
    body: &str,
    initial_text: &str,
    confirm_label: &str,
    on_confirm: F,
) where
    F: Fn(String) + 'static,
{
    let entry = gtk::Entry::new();
    entry.set_text(initial_text);
    entry.set_activates_default(true);
    entry.add_css_class("dialog-entry");

    let dialog = adw::AlertDialog::builder()
        .heading(heading)
        .body(body)
        .extra_child(&entry)
        .default_response("confirm")
        .close_response("cancel")
        .build();

    dialog.add_response("cancel", "Cancelar");
    dialog.add_response("confirm", confirm_label);
    dialog.set_response_appearance("confirm", adw::ResponseAppearance::Suggested);

    let entry_clone = entry.clone();
    dialog.connect_response(Some("confirm"), move |_, _| {
        let value = entry_clone.text().trim().to_string();
        if !value.is_empty() {
            on_confirm(value);
        }
    });

    let dialog_clone = dialog.clone();
    entry.connect_activate(move |_| {
        dialog_clone.response("confirm");
        dialog_clone.close();
    });

    dialog.present(Some(parent));
    entry.grab_focus();
    entry.select_region(0, -1);
}
