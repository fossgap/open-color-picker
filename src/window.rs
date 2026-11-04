use crate::i18n::{tr, trf, Text};
use crate::{
    background::{SessionLock, TrayRecovery},
    capture,
    color::Color,
    live, picker, preferences,
    shortcuts::{self, Action},
    store::Store,
    tray::Tray,
};
use gtk::{atk::prelude::*, cairo, gdk, gio, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};

pub struct Window {
    pub widget: gtk::ApplicationWindow,
    store: RefCell<Store>,
    shortcut_store: RefCell<shortcuts::Store>,
    shortcut_dialog: RefCell<Option<gtk::Dialog>>,
    about_dialog: RefCell<Option<gtk::AboutDialog>>,
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
    live_capture: RefCell<Option<Rc<live::Request>>>,
    picker: RefCell<Option<gtk::ApplicationWindow>>,
    pending: RefCell<Option<glib::SourceId>>,
    indicator: RefCell<Option<Tray>>,
    session_lock: SessionLock,
    tray_recovery: RefCell<TrayRecovery>,
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
        let pick = gtk::Button::with_label(tr(Text::PickColor));
        pick.set_image(Some(&gtk::Image::from_icon_name(
            Some("color-select-symbolic"),
            gtk::IconSize::Button,
        )));
        pick.set_always_show_image(true);
        pick.style_context().add_class("suggested-action");
        header.pack_start(&pick);
        let menu = gio::Menu::new();
        menu.append(Some(tr(Text::KeyboardShortcuts)), Some("app.shortcuts"));
        menu.append(Some(tr(Text::HideToTray)), Some("app.hide"));
        menu.append(Some(tr(Text::About)), Some("app.about"));
        menu.append(Some(tr(Text::Quit)), Some("app.quit"));
        let menu_button = gtk::MenuButton::new();
        menu_button.set_image(Some(&gtk::Image::from_icon_name(
            Some("open-menu-symbolic"),
            gtk::IconSize::Button,
        )));
        menu_button.set_tooltip_text(Some(tr(Text::ApplicationMenu)));
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
            a.set_name(tr(Text::CurrentPreview));
        }
        column.pack_start(&preview, false, false, 0);
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let adjust = gtk::Button::with_label(tr(Text::AdjustColorEllipsis));
        let favorite = gtk::Button::with_label(tr(Text::SaveColor));
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
            copies[row].set_tooltip_text(Some(&trf(Text::CopyFormat, &[("format", name)])));
            if let Some(a) = copies[row].accessible() {
                a.set_name(&trf(Text::CopyFormat, &[("format", name)]));
            }
            grid.attach(&label, 0, row as i32, 1, 1);
            grid.attach(&entries[row], 1, row as i32, 1, 1);
            grid.attach(&copies[row], 2, row as i32, 1, 1);
        }
        column.pack_start(&grid, false, false, 0);
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
        let (history_box, history, history_empty) = palette_page(tr(Text::RecentEmpty));
        let (favorites_box, favorites, favorites_empty) = palette_page(tr(Text::FavoritesEmpty));
        stack.add_titled(&history_box, "recent", tr(Text::Recent));
        stack.add_titled(&favorites_box, "favorites", tr(Text::Favorites));
        column.pack_start(&stack, true, true, 0);
        let status = gtk::Label::new(Some(tr(Text::PickHint)));
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
            shortcut_store: RefCell::new(shortcuts::Store::load()),
            shortcut_dialog: RefCell::new(None),
            about_dialog: RefCell::new(None),
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
            live_capture: RefCell::new(None),
            picker: RefCell::new(None),
            pending: RefCell::new(None),
            indicator: RefCell::new(None),
            session_lock: SessionLock::new(),
            tray_recovery: RefCell::new(TrayRecovery::default()),
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
        for (name, callback) in [
            (Action::Pick, Self::start_pick as fn(&Rc<Self>)),
            (Action::CopyHex, Self::copy_hex),
            (Action::CopyRgb, Self::copy_rgb),
            (Action::CopyHsl, Self::copy_hsl),
            (Action::Favorite, Self::toggle_favorite),
            (Action::Adjust, Self::adjust),
            (Action::Shortcuts, Self::shortcuts),
            (Action::About, Self::about),
            (Action::Hide, Self::hide_to_tray),
        ] {
            let action = gio::SimpleAction::new(name.id(), None);
            action.connect_activate(glib::clone!(@weak ui => move |_,_| callback(&ui)));
            app.add_action(&action);
        }
        let quit = gio::SimpleAction::new("quit", None);
        quit.connect_activate(
            glib::clone!(@weak ui, @weak app => move |_,_| { ui.cancel_capture(); app.quit(); }),
        );
        app.add_action(&quit);
        let show = gio::SimpleAction::new("show", None);
        show.connect_activate(glib::clone!(@weak ui => move |_, _| ui.present()));
        app.add_action(&show);
        let unavailable = gio::SimpleAction::new("tray-unavailable", None);
        unavailable.connect_activate(glib::clone!(@weak ui => move |_, _| {
            ui.recover_hidden_window();
        }));
        app.add_action(&unavailable);
        let weak = Rc::downgrade(&ui);
        glib::timeout_add_local(Duration::from_millis(250), move || {
            let Some(ui) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            ui.recover_hidden_window();
            glib::ControlFlow::Continue
        });
        ui.apply_shortcuts();
        ui.create_tray(app);
        ui.refresh();
        let warnings: Vec<_> = [
            ui.store.borrow().warning.as_deref(),
            ui.shortcut_store.borrow().warning.as_deref(),
        ]
        .into_iter()
        .flatten()
        .map(str::to_owned)
        .collect();
        if !warnings.is_empty() {
            ui.show_error(&warnings.join("\n"));
        }
        ui
    }

    fn recover_hidden_window(&self) {
        if self.tray_recovery.borrow_mut().should_restore(
            self.has_tray() || self.widget.is_visible() || self.busy.get(),
            self.session_lock.is_locked(),
            Instant::now(),
        ) {
            self.present();
            self.status.set_text(tr(Text::NoTray));
        }
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
            Err(error) => self.show_error(&trf(
                Text::SaveColorsError,
                &[("error", &error.to_string())],
            )),
        }
    }
    fn select(self: &Rc<Self>, color: Color) {
        self.store.borrow_mut().select(color);
        self.persist();
        self.refresh();
        self.status
            .set_text(&trf(Text::Selected, &[("color", &color.to_string())]));
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
            tr(Text::FavoriteSaved)
        } else {
            tr(Text::FavoriteRemoved)
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
            a.set_name(&trf(Text::CurrentColor, &[("color", &current.to_string())]));
        }
        let saved = store.palette.favorites.contains(&current);
        self.favorite.set_label(if saved {
            tr(Text::RemoveFavorite)
        } else {
            tr(Text::SaveColor)
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
            button.set_tooltip_text(Some(&trf(
                Text::SelectColor,
                &[("color", &color.to_string())],
            )));
            if let Some(a) = button.accessible() {
                a.set_name(&trf(Text::SelectColor, &[("color", &color.to_string())]));
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
    fn copy_rgb(self: &Rc<Self>) {
        self.copy(1);
    }
    fn copy_hsl(self: &Rc<Self>) {
        self.copy(2);
    }
    fn copy(&self, row: usize) {
        let color = self.store.borrow().palette.current;
        let value = [color.hex(), color.rgb(), color.hsl()][row].clone();
        let clipboard = gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD);
        clipboard.set_text(&value);
        clipboard.store();
        self.status
            .set_text(&trf(Text::Copied, &[("value", &value)]));
    }
    fn adjust(self: &Rc<Self>) {
        if self.busy.get() {
            return;
        }
        let dialog = gtk::ColorChooserDialog::new(Some(tr(Text::AdjustColor)), Some(&self.widget));
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
        if let Some(dialog) = self.shortcut_dialog.borrow().as_ref() {
            dialog.present();
            return;
        }
        if let Some(dialog) = self.about_dialog.borrow().as_ref() {
            dialog.present();
            return;
        }
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
                let request = live::start(move |result| {
                    let Some(ui) = weak.upgrade() else {
                        return;
                    };
                    ui.live_capture.borrow_mut().take();
                    match result {
                        live::Outcome::Ready(image, stream) => ui.open_picker(image, Some(stream)),
                        live::Outcome::Unavailable => ui.capture_still(),
                        live::Outcome::Cancelled => {
                            ui.end_pick();
                            ui.status.set_text(tr(Text::PickingCancelled));
                        }
                    }
                });
                *ui.live_capture.borrow_mut() = request;
            },
        ));
    }
    fn capture_still(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        let request = capture::capture(move |result| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            ui.capture.borrow_mut().take();
            match result {
                Ok(Some(image)) => ui.open_picker(image, None),
                Ok(None) => {
                    ui.end_pick();
                    ui.status.set_text(tr(Text::PickingCancelled));
                }
                Err(error) => {
                    ui.end_pick();
                    ui.show_error(&error);
                }
            }
        });
        *self.capture.borrow_mut() = request;
    }
    fn open_picker(self: &Rc<Self>, image: gtk::gdk_pixbuf::Pixbuf, stream: Option<live::Stream>) {
        let weak = Rc::downgrade(self);
        let picker = picker::open_with_stream(
            &self.widget.application().unwrap(),
            image,
            self.shortcut_store.borrow().bindings.clone(),
            stream,
            move |color| {
                if let Some(ui) = weak.upgrade() {
                    ui.picker.borrow_mut().take();
                    ui.end_pick();
                    if let Some(color) = color {
                        ui.select(color);
                        ui.copy(0);
                    } else {
                        ui.status.set_text(tr(Text::PickingCancelled));
                    }
                }
            },
        );
        *self.picker.borrow_mut() = Some(picker);
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
        let live = self.live_capture.borrow_mut().take();
        if let Some(live) = live {
            live.cancel();
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
        if self.busy.get() {
            return;
        }
        if let Some(dialog) = self.shortcut_dialog.borrow().as_ref() {
            dialog.present();
            return;
        }
        let dialog = preferences::shortcuts(
            &self.widget,
            self.shortcut_store.borrow().bindings.clone(),
            glib::clone!(@weak self as ui => @default-return Err(tr(Text::WindowClosed).into()), move |bindings| {
                {
                    let mut store = ui.shortcut_store.borrow_mut();
                    store.save(bindings).map_err(|error| trf(Text::SaveShortcutsError, &[("path", &store.path().display().to_string()), ("error", &error.to_string())]))?;
                }
                ui.apply_shortcuts();
                ui.status.set_text(tr(Text::ShortcutsSaved));
                Ok(())
            }),
        );
        dialog.connect_destroy(
            glib::clone!(@weak self as ui => move |_| { ui.shortcut_dialog.borrow_mut().take(); }),
        );
        *self.shortcut_dialog.borrow_mut() = Some(dialog.clone());
        dialog.show_all();
    }
    fn about(self: &Rc<Self>) {
        if self.busy.get() {
            return;
        }
        if let Some(dialog) = self.about_dialog.borrow().as_ref() {
            dialog.present();
            return;
        }
        let dialog = preferences::about(&self.widget);
        dialog.connect_destroy(
            glib::clone!(@weak self as ui => move |_| { ui.about_dialog.borrow_mut().take(); }),
        );
        *self.about_dialog.borrow_mut() = Some(dialog.clone());
        dialog.show();
    }
    fn apply_shortcuts(&self) {
        let Some(app) = self.widget.application() else {
            return;
        };
        let store = self.shortcut_store.borrow();
        for action in shortcuts::APPLICATION {
            let accelerator = store.bindings.accelerator(action);
            let accelerators = if accelerator.is_empty() {
                vec![]
            } else {
                vec![accelerator]
            };
            app.set_accels_for_action(&format!("app.{}", action.id()), &accelerators);
        }
        let shortcut = store.bindings.accelerator(Action::Pick);
        self.pick.set_tooltip_text(Some(&if shortcut.is_empty() {
            tr(Text::PickScreen).into()
        } else {
            trf(
                Text::PickScreenWithShortcut,
                &[("shortcut", &store.bindings.label(Action::Pick))],
            )
        }));
    }
    pub fn hide_to_tray(self: &Rc<Self>) {
        if self.has_tray() {
            self.widget.hide();
        } else {
            self.present();
            self.status.set_text(tr(Text::NoTray));
        }
    }
    pub fn has_tray(&self) -> bool {
        self.indicator
            .borrow()
            .as_ref()
            .is_some_and(Tray::is_available)
    }
    fn create_tray(self: &Rc<Self>, app: &gtk::Application) {
        *self.indicator.borrow_mut() = Tray::new(app).ok();
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

#[cfg(test)]
#[path = "window_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "tray_tests.rs"]
mod tray_tests;
