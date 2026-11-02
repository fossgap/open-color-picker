use crate::i18n::{tr, trf, Text};
use crate::shortcuts::{self, Action, Bindings};
use gtk::{atk::prelude::*, glib, prelude::*};
use std::{cell::RefCell, rc::Rc};

fn description(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_xalign(0.0);
    label.set_line_wrap(true);
    label.set_line_wrap_mode(gtk::pango::WrapMode::WordChar);
    label.set_max_width_chars(58);
    label
}

fn error_message(name: &str) -> (gtk::Box, gtk::Label) {
    let area = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    area.set_no_show_all(true);
    let icon = gtk::Image::from_icon_name(Some("dialog-error-symbolic"), gtk::IconSize::Button);
    icon.set_valign(gtk::Align::Start);
    let label = description("");
    label.set_widget_name(name);
    label.set_hexpand(true);
    area.pack_start(&icon, false, false, 0);
    area.pack_start(&label, true, true, 0);
    icon.show();
    label.show();
    if let Some(accessible) = area.accessible() {
        accessible.set_role(gtk::atk::Role::Alert);
    }
    (area, label)
}

fn binding_button(button: &gtk::Button, action: Action, bindings: &Bindings) {
    let shortcut = bindings.label(action);
    button.set_label(&shortcut);
    button.set_tooltip_text(Some(&trf(
        Text::ChangeShortcut,
        &[("action", action.title())],
    )));
    if let Some(accessible) = button.accessible() {
        accessible.set_name(&trf(
            Text::ShortcutAccessible,
            &[("action", action.title()), ("shortcut", &shortcut)],
        ));
    }
}

pub fn shortcuts(
    parent: &gtk::ApplicationWindow,
    initial: Bindings,
    save: impl Fn(&Bindings) -> Result<(), String> + 'static,
) -> gtk::Dialog {
    let dialog = gtk::Dialog::builder()
        .title(tr(Text::KeyboardShortcuts))
        .transient_for(parent)
        .destroy_with_parent(true)
        .modal(true)
        .use_header_bar(1)
        .default_width(570)
        .default_height(600)
        .build();
    dialog.add_button(tr(Text::Cancel), gtk::ResponseType::Cancel);
    let apply = dialog.add_button(tr(Text::Save), gtk::ResponseType::Apply);
    apply.style_context().add_class("suggested-action");
    let body = dialog.content_area();
    body.set_border_width(16);
    body.set_spacing(12);
    body.pack_start(
        &description(tr(Text::ShortcutInstructions)),
        false,
        false,
        0,
    );
    let draft = Rc::new(RefCell::new(initial));
    let stack = gtk::Stack::new();
    stack.set_widget_name("shortcuts-stack");
    let switcher = gtk::StackSwitcher::new();
    switcher.set_stack(Some(&stack));
    switcher.set_halign(gtk::Align::Center);
    body.pack_start(&switcher, false, false, 0);
    let mut buttons = Vec::new();
    for (name, title, actions) in [
        ("application", tr(Text::Application), shortcuts::APPLICATION),
        ("picker", tr(Text::WhilePicking), shortcuts::PICKER),
    ] {
        let grid = gtk::Grid::builder()
            .row_spacing(8)
            .column_spacing(16)
            .border_width(8)
            .build();
        for (index, action) in actions.into_iter().enumerate() {
            let label = description(action.title());
            label.set_hexpand(true);
            let button = gtk::Button::new();
            button.set_widget_name(&format!("shortcut-{}", action.id()));
            button.set_size_request(150, -1);
            binding_button(&button, action, &draft.borrow());
            label.set_mnemonic_widget(Some(&button));
            button.connect_clicked(glib::clone!(@weak dialog, @strong draft => move |button| {
                record(&dialog, action, draft.clone(), button);
            }));
            grid.attach(&label, 0, index as i32, 1, 1);
            grid.attach(&button, 1, index as i32, 1, 1);
            buttons.push((action, button));
        }
        let scroll = gtk::ScrolledWindow::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
        scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        scroll.set_overlay_scrolling(false);
        scroll.set_vexpand(true);
        scroll.add(&grid);
        stack.add_titled(&scroll, name, title);
    }
    body.pack_start(&stack, true, true, 0);
    let restore = gtk::Button::with_label(tr(Text::RestoreDefaults));
    restore.set_halign(gtk::Align::Start);
    restore.set_widget_name("shortcuts-restore");
    restore.connect_clicked(glib::clone!(@strong draft => move |_| {
        *draft.borrow_mut() = Bindings::default();
        for (action, button) in &buttons { binding_button(button, *action, &draft.borrow()); }
    }));
    body.pack_start(&restore, false, false, 0);
    body.pack_start(&description(tr(Text::ShortcutFooter)), false, false, 0);
    let (error_area, error) = error_message("shortcuts-save-error");
    body.pack_start(&error_area, false, false, 0);
    dialog.connect_response(
        glib::clone!(@strong draft, @weak error, @weak error_area => move |dialog, response| {
            if response == gtk::ResponseType::Apply {
                if let Err(message) = save(&draft.borrow()) {
                    error.set_text(&message);
                    error_area.show();
                    return;
                }
            }
            dialog.close();
        }),
    );
    dialog
}

fn record(
    parent: &gtk::Dialog,
    action: Action,
    draft: Rc<RefCell<Bindings>>,
    button: &gtk::Button,
) {
    let dialog = gtk::Dialog::builder()
        .title(trf(Text::ShortcutFor, &[("action", action.title())]))
        .transient_for(parent)
        .destroy_with_parent(true)
        .modal(true)
        .use_header_bar(1)
        .default_width(430)
        .resizable(false)
        .build();
    dialog.add_button(tr(Text::Cancel), gtk::ResponseType::Cancel);
    let set = dialog.add_button(tr(Text::SetShortcut), gtk::ResponseType::Ok);
    set.style_context().add_class("suggested-action");
    set.set_sensitive(false);
    let body = dialog.content_area();
    body.set_border_width(24);
    body.set_spacing(12);
    body.pack_start(
        &description(tr(Text::RecorderInstructions)),
        false,
        false,
        0,
    );
    let entry = gtk::Entry::new();
    entry.set_widget_name("shortcut-recorder");
    entry.set_placeholder_text(Some(tr(Text::PressShortcut)));
    entry.set_editable(false);
    entry.set_width_chars(30);
    if let Some(accessible) = entry.accessible() {
        accessible.set_name(&trf(Text::NewShortcut, &[("action", action.title())]));
    }
    body.pack_start(&entry, false, false, 0);
    let (error_area, error) = error_message("shortcut-conflict");
    body.pack_start(&error_area, false, false, 0);
    let disable = gtk::Button::with_label(tr(Text::DisableShortcut));
    disable.set_halign(gtk::Align::Start);
    disable.set_widget_name("shortcut-disable");
    disable.connect_clicked(
        glib::clone!(@weak dialog => move |_| dialog.response(gtk::ResponseType::Other(1))),
    );
    body.pack_start(&disable, false, false, 0);
    let pending = Rc::new(RefCell::new(None::<String>));
    dialog.connect_key_press_event(glib::clone!(@weak entry, @weak error, @weak error_area, @weak set, @strong draft, @strong pending => @default-return glib::Propagation::Proceed, move |_, event| {
        if !entry.has_focus() || matches!(event.keyval(), gtk::gdk::keys::constants::Tab | gtk::gdk::keys::constants::ISO_Left_Tab) {
            return glib::Propagation::Proceed;
        }
        if let Some(accelerator) = shortcuts::from_event(event) {
            let (key, mods) = gtk::accelerator_parse(&accelerator);
            entry.set_text(gtk::accelerator_get_label(key, mods).as_deref().unwrap_or(&accelerator));
            match draft.borrow().clone().set(action, &accelerator) {
                Ok(()) => { *pending.borrow_mut() = Some(accelerator); error_area.hide(); set.set_sensitive(true); }
                Err(message) => { pending.borrow_mut().take(); error.set_text(&message); error_area.show(); set.set_sensitive(false); }
            }
        }
        // Recording must not trigger an existing application accelerator.
        glib::Propagation::Stop
    }));
    dialog.connect_response(
        glib::clone!(@strong draft, @strong pending, @weak button => move |dialog, response| {
            let accelerator = match response {
                gtk::ResponseType::Ok => pending.borrow().clone(),
                gtk::ResponseType::Other(1) => Some(String::new()),
                _ => None,
            };
            if let Some(accelerator) = accelerator {
                if draft.borrow_mut().set(action, &accelerator).is_err() { return; }
                binding_button(&button, action, &draft.borrow());
            }
            dialog.close();
        }),
    );
    dialog.show_all();
    entry.grab_focus();
}

pub fn about(parent: &gtk::ApplicationWindow) -> gtk::AboutDialog {
    // The native constructor coordinates the About dialog's navigation with
    // GTK's header-bar setting; forcing a header bar through the generic
    // builder can expose both legacy buttons and the stack switcher.
    let dialog = gtk::AboutDialog::new();
    dialog.set_transient_for(Some(parent));
    dialog.set_destroy_with_parent(true);
    dialog.set_modal(true);
    dialog.set_program_name("Open Color Picker");
    dialog.set_version(Some(env!("CARGO_PKG_VERSION")));
    dialog.set_comments(Some(tr(Text::AboutComments)));
    dialog.set_website(Some(env!("CARGO_PKG_REPOSITORY")));
    dialog.set_website_label(Some(tr(Text::ProjectWebsite)));
    dialog.set_authors(&[tr(Text::Contributors)]);
    dialog.set_license_type(gtk::License::Gpl30Only);
    dialog.set_wrap_license(true);
    dialog.set_logo_icon_name(Some(crate::APP_ID));
    // Use the real icon even in an uninstalled `cargo run` checkout.
    if let Ok(loader) = gtk::gdk_pixbuf::PixbufLoader::with_type("svg") {
        loader.set_size(96, 96);
        if loader
            .write(include_bytes!(
                "../data/io.github.opencolorpicker.OpenColorPicker.svg"
            ))
            .is_ok()
            && loader.close().is_ok()
        {
            dialog.set_logo(loader.pixbuf().as_ref());
        }
    }
    dialog.add_credit_section(
        tr(Text::BuiltWith),
        &[
            "Rust https://www.rust-lang.org",
            "GTK https://www.gtk.org",
            "gtk-rs https://gtk-rs.org",
        ],
    );
    // Setting the program name resets GTK's default window title.
    dialog.set_title(tr(Text::About));
    dialog.connect_response(|dialog, _| dialog.close());
    dialog
}
