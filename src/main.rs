mod capture;
mod color;
mod picker;
mod pixels;
mod preferences;
mod shortcuts;
mod store;
mod tray;
mod window;

use gtk::{gio, glib, prelude::*};
use std::{cell::RefCell, rc::Rc};

pub const APP_ID: &str = "io.github.opencolorpicker.OpenColorPicker";

fn main() -> glib::ExitCode {
    glib::set_application_name("Open Color Picker");
    glib::set_prgname(Some("open-color-picker"));
    let app = gtk::Application::new(Some(APP_ID), gio::ApplicationFlags::HANDLES_COMMAND_LINE);
    for (name, description) in [
        ("pick", "Pick a color immediately"),
        ("tray", "Start in the system tray"),
        ("quit", "Quit the running application"),
        ("version", "Print the version"),
    ] {
        app.add_main_option(
            name,
            glib::Char::from(0),
            glib::OptionFlags::NONE,
            glib::OptionArg::None,
            description,
            None,
        );
    }
    app.connect_handle_local_options(|_, options| {
        if options.contains("version") {
            println!("Open Color Picker {}", env!("CARGO_PKG_VERSION"));
            0
        } else {
            -1
        }
    });
    app.connect_startup(|_| {
        gtk::Window::set_default_icon_name(APP_ID);
        let css = gtk::CssProvider::new();
        css.load_from_data(b"entry.color-value { font-family: monospace; font-size: 14px; }")
            .expect("valid built-in CSS");
        if let Some(screen) = gtk::gdk::Screen::default() {
            gtk::StyleContext::add_provider_for_screen(
                &screen,
                &css,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    });
    let ui: Rc<RefCell<Option<Rc<window::Window>>>> = Rc::new(RefCell::new(None));
    app.connect_command_line(glib::clone!(@strong ui => move |app,command| {
        let options = command.options_dict();
        if options.contains("quit") { if let Some(ui) = ui.borrow().as_ref() { ui.cancel_capture(); } app.quit(); return 0; }
        if ui.borrow().is_none() { *ui.borrow_mut() = Some(window::Window::new(app)); }
        let ui = ui.borrow().as_ref().unwrap().clone();
        if options.contains("pick") { ui.start_pick(); }
        else if options.contains("tray") { ui.hide_to_tray(); }
        else { ui.present(); }
        0
    }));
    app.connect_activate(glib::clone!(@strong ui => move |app| {
        if ui.borrow().is_none() { *ui.borrow_mut() = Some(window::Window::new(app)); }
        ui.borrow().as_ref().unwrap().present();
    }));
    app.run()
}
