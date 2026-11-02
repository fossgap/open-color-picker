use super::*;
use std::time::{Duration, Instant};

fn settle() {
    let deadline = Instant::now() + Duration::from_millis(100);
    let context = glib::MainContext::default();
    while Instant::now() < deadline {
        while context.pending() {
            context.iteration(false);
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn named(root: &impl IsA<gtk::Widget>, name: &str) -> gtk::Widget {
    fn find(widget: &gtk::Widget, name: &str) -> Option<gtk::Widget> {
        if widget.widget_name() == name {
            return Some(widget.clone());
        }
        widget
            .clone()
            .downcast::<gtk::Container>()
            .ok()
            .and_then(|container| {
                container
                    .children()
                    .into_iter()
                    .find_map(|child| find(&child, name))
            })
    }
    find(root.as_ref(), name).unwrap_or_else(|| panic!("Missing widget {name}"))
}

fn click(root: &impl IsA<gtk::Widget>, name: &str) {
    named(root, name)
        .downcast::<gtk::Button>()
        .unwrap()
        .emit_clicked();
    settle();
}

fn press(window: &impl IsA<gtk::Window>, keys: &str) {
    let window: &gtk::Window = window.as_ref();
    window.present();
    window.window().unwrap().focus(0);
    settle();
    assert!(std::process::Command::new("xdotool")
        .args(["key", "--clearmodifiers", keys])
        .status()
        .unwrap()
        .success());
    settle();
}

fn capture(window: &impl IsA<gtk::Window>, name: &str) {
    let window: &gtk::Window = window.as_ref();
    let Some(directory) = std::env::var_os("OCP_UI_CAPTURE_DIR") else {
        return;
    };
    let directory = std::path::PathBuf::from(directory);
    std::fs::create_dir_all(&directory).unwrap();
    settle();
    let pixels = window
        .window()
        .unwrap()
        .pixbuf(0, 0, window.allocated_width(), window.allocated_height())
        .unwrap();
    let theme = std::env::var("OCP_UI_THEME_LABEL").unwrap_or_else(|_| "light".into());
    pixels
        .savev(directory.join(format!("{name}-{theme}.png")), "png", &[])
        .unwrap();
}

fn editor(ui: &Rc<Window>) -> gtk::Dialog {
    ui.shortcuts();
    settle();
    ui.shortcut_dialog.borrow().as_ref().unwrap().clone()
}

fn recorder(dialog: &gtk::Dialog, action: Action) -> gtk::Dialog {
    click(dialog, &format!("shortcut-{}", action.id()));
    gtk::Window::list_toplevels()
        .into_iter()
        .find_map(|widget| {
            let dialog = widget.downcast::<gtk::Dialog>().ok()?;
            (dialog.title().as_deref() == Some(&format!("Shortcut for {}", action.title())))
                .then_some(dialog)
        })
        .unwrap()
}

fn bind(dialog: &gtk::Dialog, action: Action, keys: &str) {
    let recorder = recorder(dialog, action);
    press(&recorder, keys);
    assert!(recorder
        .widget_for_response(gtk::ResponseType::Ok)
        .unwrap()
        .is_sensitive());
    recorder.response(gtk::ResponseType::Ok);
    settle();
}

#[test]
#[ignore = "requires a private Xvfb display, D-Bus session and XDG config/data directories"]
fn preferences_dialogs() {
    // The test runner must never read or write the developer's real settings.
    assert!(std::env::var_os("XDG_CONFIG_HOME").is_some());
    assert!(std::env::var_os("XDG_DATA_HOME").is_some());
    gtk::init().unwrap();
    let app = gtk::Application::new(
        Some("io.github.opencolorpicker.PreferencesTest"),
        gio::ApplicationFlags::NON_UNIQUE,
    );
    app.register(gio::Cancellable::NONE).unwrap();
    let ui = Window::new(&app);
    ui.present();
    settle();
    let dialog = editor(&ui);
    capture(&dialog, "shortcuts-application");
    dialog.resize(440, 480);
    capture(&dialog, "shortcuts-compact");
    dialog.resize(570, 600);
    let stack = named(&dialog, "shortcuts-stack")
        .downcast::<gtk::Stack>()
        .unwrap();
    stack.set_visible_child_name("picker");
    capture(&dialog, "shortcuts-picker");
    stack.set_visible_child_name("application");
    bind(&dialog, Action::Pick, "ctrl+alt+p");
    let conflict = recorder(&dialog, Action::CopyHex);
    press(&conflict, "ctrl+alt+p");
    assert!(!conflict
        .widget_for_response(gtk::ResponseType::Ok)
        .unwrap()
        .is_sensitive());
    let error = named(&conflict, "shortcut-conflict")
        .downcast::<gtk::Label>()
        .unwrap();
    assert!(error.is_drawable() && error.text().contains("Pick a color"));
    capture(&conflict, "shortcut-conflict");
    let quit_called = Rc::new(Cell::new(false));
    app.lookup_action("quit")
        .unwrap()
        .downcast::<gio::SimpleAction>()
        .unwrap()
        .connect_activate(glib::clone!(@strong quit_called => move |_,_| quit_called.set(true)));
    press(&conflict, "ctrl+q");
    assert!(
        !quit_called.get(),
        "Recording a shortcut must not activate Quit"
    );
    assert!(error.text().contains("Quit"));
    press(&conflict, "ctrl+alt+c");
    assert!(!error.is_drawable(), "A valid shortcut clears the conflict");
    assert!(conflict
        .widget_for_response(gtk::ResponseType::Ok)
        .unwrap()
        .is_sensitive());
    conflict.response(gtk::ResponseType::Ok);
    settle();
    bind(&dialog, Action::CopyRgb, "super+r");
    dialog.response(gtk::ResponseType::Apply);
    settle();
    let close_deadline = Instant::now() + Duration::from_secs(2);
    while ui.shortcut_dialog.borrow().is_some() && Instant::now() < close_deadline {
        settle();
    }
    assert!(
        ui.shortcut_dialog.borrow().is_none(),
        "Save failed: {}. Status: {}. Dialogs: {:?}",
        named(&dialog, "shortcuts-save-error")
            .downcast::<gtk::Label>()
            .unwrap()
            .text(),
        ui.status.text(),
        gtk::Window::list_toplevels()
            .into_iter()
            .filter_map(|w| w.downcast::<gtk::Window>().ok())
            .map(|w| (w.title(), w.is_visible()))
            .collect::<Vec<_>>()
    );
    let loaded = shortcuts::Store::load();
    assert_eq!(
        gtk::accelerator_parse(loaded.bindings.accelerator(Action::Pick)),
        gtk::accelerator_parse("<Primary><Alt>p")
    );
    assert_eq!(app.accels_for_action("app.copy-hex").len(), 1);
    assert_eq!(
        gtk::accelerator_parse(&app.accels_for_action("app.copy-hex")[0]),
        gtk::accelerator_parse("<Primary><Alt>c")
    );
    assert!(ui
        .pick
        .tooltip_text()
        .unwrap()
        .contains(&loaded.bindings.label(Action::Pick)));
    let clipboard = gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD);
    clipboard.set_text("unchanged");
    ui.pick.grab_focus();
    press(&ui.widget, "ctrl+shift+c");
    assert_eq!(clipboard.wait_for_text().as_deref(), Some("unchanged"));
    press(&ui.widget, "ctrl+alt+c");
    assert_eq!(clipboard.wait_for_text().as_deref(), Some("#3584E4"));
    press(&ui.widget, "super+r");
    assert_eq!(
        clipboard.wait_for_text().as_deref(),
        Some("rgb(53, 132, 228)"),
        "Super binding: {:?}",
        app.accels_for_action("app.copy-rgb")
    );
    press(&ui.widget, "ctrl+alt+p");
    let deadline = Instant::now() + Duration::from_secs(3);
    while ui.picker.borrow().is_none() && Instant::now() < deadline {
        settle();
    }
    let picker = ui
        .picker
        .borrow()
        .as_ref()
        .expect("Custom Pick shortcut must open the picker")
        .clone();
    press(&picker, "Escape");
    assert!(ui.picker.borrow().is_none());

    let dialog = editor(&ui);
    let disable = recorder(&dialog, Action::CopyHex);
    click(&disable, "shortcut-disable");
    dialog.response(gtk::ResponseType::Apply);
    settle();
    assert!(app.accels_for_action("app.copy-hex").is_empty());
    assert_eq!(
        shortcuts::Store::load()
            .bindings
            .accelerator(Action::CopyHex),
        ""
    );
    clipboard.set_text("disabled");
    ui.pick.grab_focus();
    press(&ui.widget, "ctrl+alt+c");
    assert_eq!(clipboard.wait_for_text().as_deref(), Some("disabled"));
    let dialog = editor(&ui);
    click(&dialog, "shortcuts-restore");
    dialog.response(gtk::ResponseType::Cancel);
    settle();
    assert!(
        app.accels_for_action("app.copy-hex").is_empty(),
        "Cancel must discard restored defaults"
    );
    let dialog = editor(&ui);
    click(&dialog, "shortcuts-restore");
    dialog.response(gtk::ResponseType::Apply);
    settle();
    assert_eq!(
        gtk::accelerator_parse(&app.accels_for_action("app.pick")[0]),
        gtk::accelerator_parse("<Primary>p")
    );

    let failure = tempfile::tempdir().unwrap();
    let blocker = failure.path().join("not-a-directory");
    std::fs::write(&blocker, "x").unwrap();
    *ui.shortcut_store.borrow_mut() = shortcuts::Store::at(blocker.join("shortcuts.json"));
    let dialog = editor(&ui);
    bind(&dialog, Action::Pick, "ctrl+alt+p");
    dialog.response(gtk::ResponseType::Apply);
    settle();
    assert!(dialog.is_visible());
    let error = named(&dialog, "shortcuts-save-error")
        .downcast::<gtk::Label>()
        .unwrap();
    assert!(error.is_drawable() && error.text().contains("Could not save shortcuts"));
    assert!(error
        .text()
        .contains(&blocker.join("shortcuts.json").display().to_string()));
    assert!(error.text().contains("writable folder"));
    assert!(error.text().contains("choose Save again"));
    assert_eq!(
        gtk::accelerator_parse(&app.accels_for_action("app.pick")[0]),
        gtk::accelerator_parse("<Primary>p")
    );
    capture(&dialog, "shortcuts-save-error");
    dialog.response(gtk::ResponseType::Cancel);
    settle();
    *ui.shortcut_store.borrow_mut() = shortcuts::Store::load();

    ui.about();
    settle();
    let about = ui.about_dialog.borrow().as_ref().unwrap().clone();
    ui.about();
    assert_eq!(ui.about_dialog.borrow().as_ref().unwrap(), &about);
    assert_eq!(about.program_name().as_deref(), Some("Open Color Picker"));
    assert_eq!(about.title().as_deref(), Some("About"));
    assert_eq!(about.version().as_deref(), Some(env!("CARGO_PKG_VERSION")));
    assert_eq!(
        about.website().as_deref(),
        Some(env!("CARGO_PKG_REPOSITORY"))
    );
    assert_eq!(about.license_type(), gtk::License::Custom);
    assert!(about.license().unwrap().contains("not selected a license"));
    assert_eq!(about.logo().unwrap().width(), 96);
    assert!(!about.authors().is_empty());
    assert_eq!(
        about.transient_for().unwrap(),
        ui.widget.clone().upcast::<gtk::Window>()
    );
    capture(&about, "about");
    about.response(gtk::ResponseType::Cancel);
    settle();
    assert!(ui.about_dialog.borrow().is_none());
    ui.widget.close();
}
