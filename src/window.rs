use crate::{capture, color::Color, picker, store::Store};
use gtk::{atk::prelude::*, cairo, gdk, gio, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

pub struct Window {
    pub widget: gtk::ApplicationWindow,
    store: RefCell<Store>,
    preview: gtk::DrawingArea,
    entries: [gtk::Entry; 3],
    favorite: gtk::Button,
    history: gtk::FlowBox,
    favorites: gtk::FlowBox,
    history_empty: gtk::Label,
    favorites_empty: gtk::Label,
    status: gtk::Label,
    error: gtk::Label,
    pick: gtk::Button,
    busy: Cell<bool>,
    capture: RefCell<Option<Rc<capture::PortalCapture>>>,
    picker: RefCell<Option<gtk::ApplicationWindow>>,
    pending: RefCell<Option<glib::SourceId>>,
    indicator: RefCell<Option<libappindicator::AppIndicator>>,
}

impl Window {
    pub fn new(app: &gtk::Application) -> Rc<Self> {
        let widget = gtk::ApplicationWindow::builder()
            .application(app)
            .title("Open Color Picker")
            .default_width(540)
            .default_height(620)
            .build();
        let header = gtk::HeaderBar::builder()
            .title("Open Color Picker")
            .show_close_button(true)
            .build();
        let pick = gtk::Button::with_label("Pick a color");
        pick.set_image(Some(&gtk::Image::from_icon_name(
            Some("color-select-symbolic"),
            gtk::IconSize::Button,
        )));
        pick.set_always_show_image(true);
        pick.set_tooltip_text(Some("Pick from your screen (Ctrl+P)"));
        pick.style_context().add_class("suggested-action");
        header.pack_start(&pick);
        let menu = gio::Menu::new();
        menu.append(Some("Keyboard shortcuts"), Some("app.shortcuts"));
        menu.append(Some("Hide to tray"), Some("app.hide"));
        menu.append(Some("About Open Color Picker"), Some("app.about"));
        menu.append(Some("Quit"), Some("app.quit"));
        let menu_button = gtk::MenuButton::new();
        menu_button.set_image(Some(&gtk::Image::from_icon_name(
            Some("open-menu-symbolic"),
            gtk::IconSize::Button,
        )));
        menu_button.set_tooltip_text(Some("Application menu"));
        menu_button.set_menu_model(Some(&menu));
        header.pack_end(&menu_button);
        widget.set_titlebar(Some(&header));

        let scroll = gtk::ScrolledWindow::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
        scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        let column = gtk::Box::new(gtk::Orientation::Vertical, 16);
        column.set_border_width(24);
        let error = gtk::Label::new(None);
        error.set_line_wrap(true);
        error.set_max_width_chars(48);
        error.set_xalign(0.0);
        error.style_context().add_class("error");
        error.set_no_show_all(true);
        column.pack_start(&error, false, false, 0);
        let preview = gtk::DrawingArea::new();
        preview.set_height_request(144);
        if let Some(a) = preview.accessible() {
            a.set_name("Current color preview");
        }
        column.pack_start(&preview, false, false, 0);
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let adjust = gtk::Button::with_label("Adjust color…");
        let favorite = gtk::Button::with_label("Save color");
        actions.pack_start(&adjust, false, false, 0);
        actions.pack_end(&favorite, false, false, 0);
        column.pack_start(&actions, false, false, 0);
        let grid = gtk::Grid::builder()
            .column_spacing(12)
            .row_spacing(10)
            .build();
        let entries: [gtk::Entry; 3] = std::array::from_fn(|_| {
            let entry = gtk::Entry::new();
            entry.set_hexpand(true);
            entry.set_width_chars(24);
            entry.style_context().add_class("color-value");
            entry
        });
        let copies: [gtk::Button; 3] = std::array::from_fn(|_| {
            gtk::Button::from_icon_name(Some("edit-copy-symbolic"), gtk::IconSize::Button)
        });
        for (row, name) in ["HEX", "RGB", "HSL"].iter().enumerate() {
            let label = gtk::Label::new(Some(name));
            label.set_xalign(0.0);
            label.set_mnemonic_widget(Some(&entries[row]));
            if let Some(a) = entries[row].accessible() {
                a.set_name(name);
            }
            copies[row].set_tooltip_text(Some(&format!("Copy {name}")));
            if let Some(a) = copies[row].accessible() {
                a.set_name(&format!("Copy {name}"));
            }
            grid.attach(&label, 0, row as i32, 1, 1);
            grid.attach(&entries[row], 1, row as i32, 1, 1);
            grid.attach(&copies[row], 2, row as i32, 1, 1);
        }
        column.pack_start(&grid, false, false, 0);
        let hint = gtk::Label::new(Some("Edit a value and press Enter to apply."));
        hint.set_xalign(0.0);
        column.pack_start(&hint, false, false, 0);
        column.pack_start(
            &gtk::Separator::new(gtk::Orientation::Horizontal),
            false,
            false,
            0,
        );
        let stack = gtk::Stack::new();
        let switcher = gtk::StackSwitcher::new();
        switcher.set_stack(Some(&stack));
        switcher.set_halign(gtk::Align::Center);
        column.pack_start(&switcher, false, false, 0);
        let (history_box, history, history_empty) =
            palette_page("Your recent colors appear here after your first pick.");
        let (favorites_box, favorites, favorites_empty) =
            palette_page("Save a color to keep it here for next time.");
        stack.add_titled(&history_box, "recent", "Recent");
        stack.add_titled(&favorites_box, "favorites", "Favorites");
        column.pack_start(&stack, true, true, 0);
        let status = gtk::Label::new(Some("Pick a color from anywhere on your screen."));
        status.set_line_wrap(true);
        status.set_max_width_chars(48);
        status.set_xalign(0.0);
        status.set_margin_start(24);
        status.set_margin_end(24);
        status.set_margin_top(12);
        status.set_margin_bottom(16);
        scroll.add(&column);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
        body.pack_start(&scroll, true, true, 0);
        body.pack_end(&status, false, false, 0);
        widget.add(&body);
        let ui = Rc::new(Self {
            widget,
            store: RefCell::new(Store::load()),
            preview,
            entries,
            favorite,
            history,
            favorites,
            history_empty,
            favorites_empty,
            status,
            error,
            pick,
            busy: Cell::new(false),
            capture: RefCell::new(None),
            picker: RefCell::new(None),
            pending: RefCell::new(None),
            indicator: RefCell::new(None),
        });

        ui.preview.connect_draw(glib::clone!(@weak ui => @default-return glib::Propagation::Proceed, move |area,cr| {
            let color = ui.store.borrow().palette.current;
            let (r,g,b) = color.normalized(); cr.set_source_rgb(r,g,b); let _ = cr.paint();
            let (r,g,b) = color.foreground(); cr.set_source_rgb(r,g,b);
            cr.select_font_face("monospace",cairo::FontSlant::Normal,cairo::FontWeight::Bold); cr.set_font_size(30.0);
            if let Ok(extent) = cr.text_extents(&color.hex()) {
                cr.move_to((f64::from(area.allocated_width())-extent.width())/2.0-extent.x_bearing(),
                    (f64::from(area.allocated_height())-extent.height())/2.0-extent.y_bearing());
                let _ = cr.show_text(&color.hex());
            }
            glib::Propagation::Stop
        }));
        ui.pick
            .connect_clicked(glib::clone!(@weak ui => move |_| ui.start_pick()));
        ui.favorite
            .connect_clicked(glib::clone!(@weak ui => move |_| ui.toggle_favorite()));
        adjust.connect_clicked(glib::clone!(@weak ui => move |_| ui.adjust()));
        for (row, copy) in copies.iter().enumerate() {
            copy.connect_clicked(glib::clone!(@weak ui => move |_| ui.copy(row)));
            ui.entries[row].connect_activate(glib::clone!(@weak ui => move |entry| {
                match entry.text().parse::<Color>() {
                    Ok(color) => ui.select(color),
                    Err(error) => { entry.style_context().add_class("error"); ui.show_error(&error); }
                }
            }));
        }
        ui.widget.connect_delete_event(glib::clone!(@weak ui => @default-return glib::Propagation::Proceed, move |_,_| {
            if ui.has_tray() { ui.widget.hide(); glib::Propagation::Stop }
            else { if let Some(app) = ui.widget.application() { app.quit(); } glib::Propagation::Proceed }
        }));
        for (name, keys, callback) in [
            (
                "pick",
                vec!["<Primary>p"],
                Self::start_pick as fn(&Rc<Self>),
            ),
            ("copy", vec!["<Primary><Shift>c"], Self::copy_hex),
            ("favorite", vec!["<Primary>s"], Self::toggle_favorite),
            ("shortcuts", vec!["F1"], Self::shortcuts),
            ("about", vec![], Self::about),
            ("hide", vec![], Self::hide_to_tray),
        ] {
            let action = gio::SimpleAction::new(name, None);
            action.connect_activate(glib::clone!(@weak ui => move |_,_| callback(&ui)));
            app.add_action(&action);
            app.set_accels_for_action(&format!("app.{name}"), &keys);
        }
        let quit = gio::SimpleAction::new("quit", None);
        quit.connect_activate(
            glib::clone!(@weak ui, @weak app => move |_,_| { ui.cancel_capture(); app.quit(); }),
        );
        app.add_action(&quit);
        app.set_accels_for_action("app.quit", &["<Primary>q"]);
        ui.create_tray(app);
        ui.refresh();
        if let Some(message) = ui.store.borrow().warning.as_ref() {
            ui.show_error(message);
        }
        ui
    }

    pub fn present(&self) {
        self.widget.show_all();
        self.widget.present();
    }
    fn show_error(&self, message: &str) {
        self.error.set_text(message);
        self.error.show();
    }
    fn persist(&self) {
        match self.store.borrow_mut().save() {
            Ok(()) => self.error.hide(),
            Err(error) => self.show_error(&format!(
                "Could not save your colors: {error}. They remain available until you quit."
            )),
        }
    }
    fn select(self: &Rc<Self>, color: Color) {
        self.store.borrow_mut().select(color);
        self.persist();
        self.refresh();
        self.status.set_text(&format!("Selected {color}."));
    }
    fn toggle_favorite(self: &Rc<Self>) {
        if self.busy.get() {
            return;
        }
        self.store.borrow_mut().toggle_favorite();
        self.persist();
        self.refresh();
        let saved = self
            .store
            .borrow()
            .palette
            .favorites
            .contains(&self.store.borrow().palette.current);
        self.status.set_text(if saved {
            "Color saved to Favorites."
        } else {
            "Color removed from Favorites."
        });
    }
    fn refresh(self: &Rc<Self>) {
        let store = self.store.borrow();
        let current = store.palette.current;
        for (entry, value) in self
            .entries
            .iter()
            .zip([current.hex(), current.rgb(), current.hsl()])
        {
            entry.set_text(&value);
            entry.style_context().remove_class("error");
        }
        self.preview.queue_draw();
        if let Some(a) = self.preview.accessible() {
            a.set_name(&format!("Current color {current}"));
        }
        let saved = store.palette.favorites.contains(&current);
        self.favorite.set_label(if saved {
            "Remove favorite"
        } else {
            "Save color"
        });
        self.favorite.set_image(Some(&gtk::Image::from_icon_name(
            Some(if saved {
                "starred-symbolic"
            } else {
                "non-starred-symbolic"
            }),
            gtk::IconSize::Button,
        )));
        self.favorite.set_always_show_image(true);
        self.render_palette(&self.history, &store.palette.history);
        self.render_palette(&self.favorites, &store.palette.favorites);
        self.history_empty
            .set_visible(store.palette.history.is_empty());
        self.favorites_empty
            .set_visible(store.palette.favorites.is_empty());
    }
    fn render_palette(self: &Rc<Self>, flow: &gtk::FlowBox, colors: &[Color]) {
        for child in flow.children() {
            flow.remove(&child);
        }
        for &color in colors {
            let button = gtk::Button::new();
            button.set_tooltip_text(Some(&format!("Select {color}")));
            if let Some(a) = button.accessible() {
                a.set_name(&format!("Select {color}"));
            }
            let swatch = gtk::DrawingArea::new();
            swatch.set_size_request(44, 36);
            swatch.connect_draw(move |_, cr| {
                let (r, g, b) = color.normalized();
                cr.set_source_rgb(r, g, b);
                let _ = cr.paint();
                glib::Propagation::Stop
            });
            button.add(&swatch);
            button.connect_clicked(glib::clone!(@weak self as ui => move |_| ui.select(color)));
            flow.insert(&button, -1);
        }
        flow.show_all();
    }
    fn copy_hex(self: &Rc<Self>) {
        self.copy(0);
    }
    fn copy(&self, row: usize) {
        let color = self.store.borrow().palette.current;
        let value = [color.hex(), color.rgb(), color.hsl()][row].clone();
        let clipboard = gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD);
        clipboard.set_text(&value);
        clipboard.store();
        self.status.set_text(&format!("Copied {value}."));
    }
    fn adjust(self: &Rc<Self>) {
        let dialog = gtk::ColorChooserDialog::new(Some("Adjust color"), Some(&self.widget));
        let (r, g, b) = self.store.borrow().palette.current.normalized();
        dialog.set_rgba(&gdk::RGBA::new(r, g, b, 1.0));
        dialog.set_use_alpha(false);
        dialog.connect_response(glib::clone!(@weak self as ui => move |dialog,response| {
            if response == gtk::ResponseType::Ok { let c = dialog.rgba(); ui.select(Color::from_normalized(c.red(),c.green(),c.blue())); }
            dialog.close();
        }));
        dialog.show();
    }
    pub fn start_pick(self: &Rc<Self>) {
        if self.busy.replace(true) {
            if let Some(picker) = self.picker.borrow().as_ref() {
                picker.present();
            }
            return;
        }
        self.pick.set_sensitive(false);
        self.widget.hide();
        let weak = Rc::downgrade(self);
        *self.pending.borrow_mut() = Some(glib::timeout_add_local_once(
            Duration::from_millis(250),
            move || {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                ui.pending.borrow_mut().take();
                let weak = Rc::downgrade(&ui);
                let request = capture::capture(move |result| {
                    let Some(ui) = weak.upgrade() else {
                        return;
                    };
                    ui.capture.borrow_mut().take();
                    match result {
                        Ok(Some(image)) => {
                            let weak = Rc::downgrade(&ui);
                            let picker = picker::open(
                                &ui.widget.application().unwrap(),
                                image,
                                move |color| {
                                    if let Some(ui) = weak.upgrade() {
                                        ui.picker.borrow_mut().take();
                                        ui.end_pick();
                                        if let Some(color) = color {
                                            ui.select(color);
                                            ui.copy(0);
                                        } else {
                                            ui.status.set_text("Picking cancelled.");
                                        }
                                    }
                                },
                            );
                            *ui.picker.borrow_mut() = Some(picker);
                        }
                        Ok(None) => {
                            ui.end_pick();
                            ui.status.set_text("Picking cancelled.");
                        }
                        Err(error) => {
                            ui.end_pick();
                            ui.show_error(&error);
                        }
                    }
                });
                *ui.capture.borrow_mut() = request;
            },
        ));
    }
    fn end_pick(&self) {
        self.busy.set(false);
        self.pick.set_sensitive(true);
        self.present();
    }
    pub fn cancel_capture(&self) {
        if let Some(id) = self.pending.borrow_mut().take() {
            id.remove();
        }
        let request = self.capture.borrow_mut().take();
        if let Some(request) = request {
            request.cancel();
        }
        let picker = self.picker.borrow_mut().take();
        if let Some(picker) = picker {
            picker.close();
        }
        self.busy.set(false);
    }
    fn shortcuts(self: &Rc<Self>) {
        let dialog = gtk::MessageDialog::builder().transient_for(&self.widget).modal(true)
            .message_type(gtk::MessageType::Info).buttons(gtk::ButtonsType::Close)
            .text("Keyboard shortcuts")
            .secondary_text("Ctrl+P   Pick a color\nCtrl+Shift+C   Copy HEX\nCtrl+S   Save or remove a favorite\nCtrl+Q   Quit\nF1   Show shortcuts\n\nWhile picking:\nArrows   Move one source pixel\nShift+Arrows   Move ten pixels\nEnter   Use color\nEsc   Cancel\n\nFor a desktop-wide shortcut, bind\nopen-color-picker --pick\nin your desktop's keyboard settings.").build();
        dialog.connect_response(|d, _| d.close());
        dialog.show();
    }
    fn about(self: &Rc<Self>) {
        let dialog = gtk::AboutDialog::builder()
            .transient_for(&self.widget)
            .modal(true)
            .program_name("Open Color Picker")
            .version(env!("CARGO_PKG_VERSION"))
            .comments(
                "Pick, copy, and keep colors from your Linux desktop.\nBuilt with Rust and GTK3.",
            )
            .logo_icon_name(crate::APP_ID)
            .build();
        dialog.connect_response(|d, _| d.close());
        dialog.show();
    }
    pub fn hide_to_tray(self: &Rc<Self>) {
        if self.has_tray() {
            self.widget.hide();
        } else {
            self.present();
            self.status
                .set_text("No system tray is available. The window will stay open.");
        }
    }
    pub fn has_tray(&self) -> bool {
        self.indicator.borrow().is_some() && tray_host_exists()
    }
    fn create_tray(self: &Rc<Self>, app: &gtk::Application) {
        let library = ["libayatana-appindicator3.so.1", "libappindicator3.so.1"]
            .iter()
            .find_map(|name| {
                // SAFETY: load a standard native GTK indicator library; no symbols are
                // invoked here. libappindicator owns its own handle for all FFI calls.
                unsafe { libloading::Library::new(name).ok() }
            });
        if library.is_none() {
            return;
        }
        let mut indicator =
            libappindicator::AppIndicator::new(crate::APP_ID, "color-select-symbolic");
        indicator.set_title("Open Color Picker");
        let mut menu = gtk::Menu::new();
        let pick = gtk::MenuItem::with_label("Pick a color");
        pick.connect_activate(glib::clone!(@weak self as ui => move |_| ui.start_pick()));
        let show = gtk::MenuItem::with_label("Show colors");
        show.connect_activate(glib::clone!(@weak self as ui => move |_| ui.present()));
        let quit = gtk::MenuItem::with_label("Quit");
        quit.connect_activate(glib::clone!(@weak self as ui, @weak app => move |_| { ui.cancel_capture(); app.quit(); }));
        menu.append(&pick);
        menu.append(&show);
        menu.append(&gtk::SeparatorMenuItem::new());
        menu.append(&quit);
        menu.show_all();
        indicator.set_menu(&mut menu);
        indicator.set_status(libappindicator::AppIndicatorStatus::Active);
        *self.indicator.borrow_mut() = Some(indicator);
    }
}

fn palette_page(empty_text: &str) -> (gtk::Box, gtk::FlowBox, gtk::Label) {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let empty = gtk::Label::new(Some(empty_text));
    empty.set_line_wrap(true);
    empty.set_max_width_chars(40);
    empty.set_margin_top(16);
    empty.set_margin_bottom(16);
    empty.set_no_show_all(true);
    let flow = gtk::FlowBox::new();
    flow.set_selection_mode(gtk::SelectionMode::None);
    flow.set_homogeneous(true);
    flow.set_column_spacing(6);
    flow.set_row_spacing(6);
    flow.set_min_children_per_line(1);
    flow.set_max_children_per_line(8);
    flow.set_valign(gtk::Align::Start);
    column.pack_start(&empty, false, false, 0);
    column.pack_start(&flow, false, false, 0);
    (column, flow, empty)
}

fn tray_host_exists() -> bool {
    gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE)
        .ok()
        .and_then(|bus| {
            bus.call_sync(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus",
                "NameHasOwner",
                Some(&("org.kde.StatusNotifierWatcher",).to_variant()),
                None,
                gio::DBusCallFlags::NONE,
                500,
                gio::Cancellable::NONE,
            )
            .ok()
        })
        .and_then(|v| v.get::<(bool,)>())
        .is_some_and(|v| v.0)
}
