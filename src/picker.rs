use crate::i18n::{tr, trf, Text};
use crate::{
    color::Color,
    live,
    pixels::{Desktop, MonitorArea, Pixels},
    shortcuts::{Action, Bindings},
};
use gtk::{atk::prelude::*, cairo, gdk, gdk_pixbuf::Pixbuf, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

type PickCallback = Box<dyn FnOnce(Option<Color>)>;

struct View {
    window: glib::WeakRef<gtk::ApplicationWindow>,
    canvas: glib::WeakRef<gtk::DrawingArea>,
    confirm: glib::WeakRef<gtk::Button>,
    help: glib::WeakRef<gtk::Label>,
    controls: glib::WeakRef<gtk::Box>,
    masks: RefCell<Vec<gdk::Rectangle>>,
}

struct Session {
    image: RefCell<Pixbuf>,
    pixels: RefCell<Pixels>,
    live: RefCell<Option<live::Stream>>,
    refresh: RefCell<Option<glib::SourceId>>,
    confirm_pending: RefCell<Option<glib::SourceId>>,
    previous_masks: RefCell<Vec<gdk::Rectangle>>,
    hint: String,
    desktop: Desktop,
    selected: Cell<(i32, i32)>,
    views: RefCell<Vec<View>>,
    callback: RefCell<Option<PickCallback>>,
    done: Cell<bool>,
    monitor_signals: RefCell<Vec<(glib::Object, glib::SignalHandlerId)>>,
}

impl Session {
    fn update(&self, point: (i32, i32)) {
        self.selected.set(point);
        let description = format!(
            "{} · x {} · y {}",
            self.pixels.borrow().at(point.0, point.1),
            point.0,
            point.1
        );
        for view in self.views.borrow().iter() {
            if let Some(help) = view.help.upgrade() {
                help.set_text(&format!(
                    "{} · {}",
                    tr(if self.live.borrow().is_some() {
                        Text::LiveDesktop
                    } else {
                        Text::FrozenDesktop
                    }),
                    self.hint
                ));
            }
            if let Some(canvas) = view.canvas.upgrade() {
                if let Some(accessible) = canvas.accessible() {
                    accessible
                        .set_name(&trf(Text::CapturedImage, &[("description", &description)]));
                }
                canvas.queue_draw();
            }
            if let Some(confirm) = view.confirm.upgrade() {
                confirm.set_tooltip_text(Some(&trf(
                    Text::UseDescription,
                    &[("description", &description)],
                )));
                if let Some(accessible) = confirm.accessible() {
                    accessible.set_description(&description);
                }
            }
        }
    }

    fn point(&self, monitor: usize, x: f64, y: f64) {
        if self.confirm_pending.borrow().is_some() {
            return;
        }
        if let Some(point) = self.desktop.source_at(monitor, x, y) {
            let previous = self.desktop.monitor_at(self.selected.get());
            self.update(point);
            if previous != Some(monitor) {
                self.focus(monitor);
            }
        }
    }

    fn draw(
        &self,
        index: usize,
        controls: &gtk::Box,
        area: &gtk::DrawingArea,
        cr: &cairo::Context,
    ) {
        let geometry = self.desktop.monitors[index];
        let _ = cr.save();
        cr.set_operator(cairo::Operator::Source);
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.0);
        let _ = cr.paint();
        let _ = cr.restore();
        if self.live.borrow().is_none() {
            let _ = cr.save();
            cr.translate(
                -f64::from(geometry.x - self.desktop.origin.0),
                -f64::from(geometry.y - self.desktop.origin.1),
            );
            cr.scale(1.0 / self.desktop.scale.0, 1.0 / self.desktop.scale.1);
            cr.set_source_pixbuf(&self.image.borrow(), 0.0, 0.0);
            cr.source().set_filter(cairo::Filter::Nearest);
            let _ = cr.paint();
            let _ = cr.restore();
        }
        let point = self.selected.get();
        let mut masks = Vec::new();
        if self.desktop.monitor_at(point) == Some(index) {
            masks = draw_magnifier(
                cr,
                (
                    area.allocated_width(),
                    area.allocated_height() - controls.allocated_height() - 32,
                ),
                &self.pixels.borrow(),
                point,
                self.desktop.local_at(index, point),
                if self.live.borrow().is_some() {
                    (
                        8.0 / self.desktop.scale.0 + 2.0,
                        8.0 / self.desktop.scale.1 + 2.0,
                    )
                } else {
                    (0.0, 0.0)
                },
            );
        }
        if self.live.borrow().is_some() {
            if let Some((x, y)) = controls.translate_coordinates(area, 0, 0) {
                masks.push((
                    f64::from(x),
                    f64::from(y),
                    f64::from(controls.allocated_width()),
                    f64::from(controls.allocated_height()),
                ));
            }
            let image = self.image.borrow();
            for (x, y, w, h) in masks {
                let left = ((x + f64::from(geometry.x - self.desktop.origin.0))
                    * self.desktop.scale.0)
                    .floor()
                    .max(0.0) as i32;
                let top = ((y + f64::from(geometry.y - self.desktop.origin.1))
                    * self.desktop.scale.1)
                    .floor()
                    .max(0.0) as i32;
                let right = ((x + w + f64::from(geometry.x - self.desktop.origin.0))
                    * self.desktop.scale.0)
                    .ceil()
                    .min(f64::from(image.width())) as i32;
                let bottom = ((y + h + f64::from(geometry.y - self.desktop.origin.1))
                    * self.desktop.scale.1)
                    .ceil()
                    .min(f64::from(image.height())) as i32;
                if right > left && bottom > top {
                    self.views.borrow()[index]
                        .masks
                        .borrow_mut()
                        .push(gdk::Rectangle::new(left, top, right - left, bottom - top));
                }
            }
        }
    }

    fn accept_frame(&self, image: Pixbuf) {
        // The desktop stream can include this application's overlay. Preserve
        // the last unobscured pixels under our UI, never feed it back into the
        // magnifier. Include the previous frame's masks for capture latency.
        let masks: Vec<_> = self
            .views
            .borrow()
            .iter()
            .flat_map(|view| view.masks.borrow_mut().drain(..).collect::<Vec<_>>())
            .collect();
        let previous = self.image.borrow();
        for rect in masks.iter().chain(self.previous_masks.borrow().iter()) {
            previous.copy_area(
                rect.x(),
                rect.y(),
                rect.width(),
                rect.height(),
                &image,
                rect.x(),
                rect.y(),
            );
        }
        drop(previous);
        *self.previous_masks.borrow_mut() = masks;
        *self.pixels.borrow_mut() = Pixels::new(&image);
        *self.image.borrow_mut() = image;
        self.update(self.selected.get());
    }

    fn avoid_controls(&self, point: (i32, i32)) {
        let Some(index) = self.desktop.monitor_at(point) else {
            return;
        };
        if let Some(controls) = self.views.borrow()[index].controls.upgrade() {
            let (_, y) = self.desktop.local_at(index, point);
            let height = self.desktop.monitors[index].height;
            controls.set_valign(
                if y > f64::from(height - controls.allocated_height() - 48) {
                    gtk::Align::Start
                } else {
                    gtk::Align::End
                },
            );
        }
    }

    fn confirm(self: &Rc<Self>, point: (i32, i32)) {
        if self.done.get() || self.confirm_pending.borrow().is_some() {
            return;
        }
        self.update(point);
        self.avoid_controls(point);
        if self.live.borrow().is_none() {
            let color = self.pixels.borrow().at(point.0, point.1);
            self.finish(Some(color));
            return;
        }
        // Let a freshly uncovered pixel arrive before confirming a click that
        // may have landed where the magnifier was drawn in the previous frame.
        *self.confirm_pending.borrow_mut() = Some(glib::timeout_add_local_once(
            std::time::Duration::from_millis(100),
            glib::clone!(@weak self as session => move || {
                session.confirm_pending.borrow_mut().take();
                let color = session.pixels.borrow().at(point.0, point.1);
                session.finish(Some(color));
            }),
        ));
    }

    fn focus(&self, monitor: usize) {
        let views = self.views.borrow();
        if let Some(window) = views[monitor].window.upgrade() {
            window.present();
        }
        if let Some(canvas) = views[monitor].canvas.upgrade() {
            canvas.grab_focus();
        }
    }

    fn finish(&self, color: Option<Color>) {
        if self.done.replace(true) {
            return;
        }
        if let Some(source) = self.refresh.borrow_mut().take() {
            source.remove();
        }
        if let Some(source) = self.confirm_pending.borrow_mut().take() {
            source.remove();
        }
        self.live.borrow_mut().take();
        for (object, id) in self.monitor_signals.borrow_mut().drain(..) {
            object.disconnect(id);
        }
        for view in self.views.borrow().iter() {
            if let Some(window) = view.window.upgrade() {
                window.hide();
                window.close();
            }
        }
        if let Some(callback) = self.callback.borrow_mut().take() {
            callback(color);
        }
    }
}

#[cfg(test)]
pub fn open(
    app: &gtk::Application,
    image: Pixbuf,
    bindings: Bindings,
    on_pick: impl FnOnce(Option<Color>) + 'static,
) -> gtk::ApplicationWindow {
    open_with_stream(app, image, bindings, None, on_pick)
}

pub fn open_with_stream(
    app: &gtk::Application,
    image: Pixbuf,
    bindings: Bindings,
    stream: Option<live::Stream>,
    on_pick: impl FnOnce(Option<Color>) + 'static,
) -> gtk::ApplicationWindow {
    // Keep a supplied first frame usable even if a caller bypasses the live
    // capture preflight. Wayland cannot reveal the desktop through fullscreen.
    let captured_areas = stream.as_ref().map(|stream| stream.areas.clone());
    let stream = stream.filter(|_| !crate::capture::is_wayland());
    let display = gdk::Display::default().expect("GTK has a display");
    let screen = display.default_screen();
    let monitors: Vec<_> = (0..display.n_monitors())
        .filter_map(|index| {
            display
                .monitor(index)
                .map(|monitor| (index, monitor.geometry()))
        })
        .filter(|(_, geometry)| {
            captured_areas.as_ref().is_none_or(|areas| {
                areas.iter().any(|a| {
                    a.x == geometry.x()
                        && a.y == geometry.y()
                        && a.width == geometry.width()
                        && a.height == geometry.height()
                })
            })
        })
        .fold(Vec::new(), |mut monitors, item| {
            // Mirrored outputs share one desktop area and need one overlay.
            if !monitors.iter().any(|(_, geometry)| *geometry == item.1) {
                monitors.push(item);
            }
            monitors
        });
    let areas = monitors
        .iter()
        .map(|(_, geometry)| MonitorArea {
            x: geometry.x(),
            y: geometry.y(),
            width: geometry.width(),
            height: geometry.height(),
        })
        .collect();
    let desktop = Desktop::new((image.width(), image.height()), areas);
    let mut active = display
        .primary_monitor()
        .and_then(|primary| {
            monitors
                .iter()
                .position(|(_, geometry)| *geometry == primary.geometry())
        })
        .unwrap_or(0);
    let geometry = desktop.monitors[active];
    let mut selected = desktop
        .source_at(
            active,
            f64::from(geometry.width) / 2.0,
            f64::from(geometry.height) / 2.0,
        )
        .unwrap();
    // Wayland does not expose a global pointer position. The overlay's enter
    // event supplies the correct local coordinates as soon as it is mapped.
    if display.type_().name().contains("X11") {
        if let Some(pointer) = display.default_seat().and_then(|seat| seat.pointer()) {
            let (_, x, y) = pointer.position();
            for (index, area) in desktop.monitors.iter().enumerate() {
                if let Some(point) =
                    desktop.source_at(index, f64::from(x - area.x), f64::from(y - area.y))
                {
                    active = index;
                    selected = point;
                    break;
                }
            }
        }
    }
    let hint = trf(
        Text::PickerOverlayHint,
        &[
            ("confirm", &bindings.label(Action::Confirm)),
            ("cancel", &bindings.label(Action::Cancel)),
        ],
    );
    let session = Rc::new(Session {
        pixels: RefCell::new(Pixels::new(&image)),
        image: RefCell::new(image),
        live: RefCell::new(stream),
        refresh: RefCell::new(None),
        confirm_pending: RefCell::new(None),
        previous_masks: RefCell::new(Vec::new()),
        hint: hint.clone(),
        desktop,
        selected: Cell::new(selected),
        views: RefCell::new(Vec::new()),
        callback: RefCell::new(Some(Box::new(on_pick))),
        done: Cell::new(false),
        monitor_signals: RefCell::new(Vec::new()),
    });
    let mut windows = Vec::new();
    let help_text = bindings.picker_help();
    let hint = trf(
        Text::PickerOverlayHint,
        &[
            ("confirm", &bindings.label(Action::Confirm)),
            ("cancel", &bindings.label(Action::Cancel)),
        ],
    );
    for (index, (monitor_index, geometry)) in monitors.iter().enumerate() {
        // Compositor map/unmap animations move our UI outside its capture masks.
        // A utility window avoids normal-window animations on X11 desktops.
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .title(tr(Text::PickPixelTitle))
            .type_hint(gdk::WindowTypeHint::Utility)
            .app_paintable(true)
            .decorated(false)
            .resizable(false)
            .skip_taskbar_hint(true)
            .skip_pager_hint(true)
            .focus_on_map(false)
            .default_width(geometry.width())
            .default_height(geometry.height())
            .build();
        if let Some(visual) = screen.rgba_visual() {
            window.set_visual(Some(&visual));
        }
        window.set_keep_above(true);
        window.move_(geometry.x(), geometry.y());
        window.fullscreen_on_monitor(&screen, *monitor_index);
        let overlay = gtk::Overlay::new();
        if let Some(accessible) = overlay.accessible() {
            accessible.set_name(tr(Text::PickPixel));
        }
        let canvas = gtk::DrawingArea::new();
        canvas.set_widget_name("picker-canvas");
        canvas.set_can_focus(true);
        canvas.set_hexpand(true);
        canvas.set_vexpand(true);
        canvas.add_events(
            gdk::EventMask::POINTER_MOTION_MASK
                | gdk::EventMask::BUTTON_PRESS_MASK
                | gdk::EventMask::ENTER_NOTIFY_MASK,
        );
        if let Some(accessible) = canvas.accessible() {
            accessible.set_description(&format!("{}\n{help_text}", tr(Text::CapturedScreen)));
        }
        overlay.add(&canvas);
        let controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        controls.set_widget_name("picker-controls");
        controls.style_context().add_class("osd");
        controls.set_halign(gtk::Align::Center);
        controls.set_valign(gtk::Align::End);
        controls.set_margin_bottom(16);
        controls.set_margin_top(16);
        controls.set_margin_start(16);
        controls.set_margin_end(16);
        controls.set_border_width(8);
        let help = gtk::Label::new(Some(&hint));
        help.set_line_wrap(true);
        help.set_max_width_chars(48);
        let cancel = gtk::Button::with_label(tr(Text::Cancel));
        cancel.set_widget_name("picker-cancel");
        let confirm = gtk::Button::with_label(tr(Text::UseColor));
        confirm.set_widget_name("picker-confirm");
        confirm.style_context().add_class("suggested-action");
        controls.pack_start(&help, false, false, 4);
        controls.pack_start(&cancel, false, false, 0);
        controls.pack_start(&confirm, false, false, 0);
        overlay.add_overlay(&controls);
        window.add(&overlay);
        session.views.borrow_mut().push(View {
            window: window.downgrade(),
            canvas: canvas.downgrade(),
            confirm: confirm.downgrade(),
            help: help.downgrade(),
            controls: controls.downgrade(),
            masks: RefCell::new(Vec::new()),
        });
        canvas.connect_realize(|canvas| {
            if let Some(window) = canvas.window() {
                window.set_cursor(gdk::Cursor::from_name(&window.display(), "none").as_ref());
            }
        });
        canvas.connect_draw(glib::clone!(@strong session, @weak controls => @default-return glib::Propagation::Proceed, move |area, cr| {
            session.draw(index, &controls, area, cr);
            glib::Propagation::Stop
        }));
        canvas.connect_enter_notify_event(glib::clone!(@strong session => @default-return glib::Propagation::Proceed, move |_, event| {
            let (x, y) = event.position();
            session.point(index, x, y);
            glib::Propagation::Stop
        }));
        canvas.connect_motion_notify_event(glib::clone!(@strong session => @default-return glib::Propagation::Proceed, move |canvas, event| {
            let (x, y) = event.position();
            session.point(index, x, y);
            canvas.grab_focus();
            glib::Propagation::Stop
        }));
        canvas.connect_button_press_event(glib::clone!(@strong session => @default-return glib::Propagation::Proceed, move |_, event| {
            if event.button() == 1 {
                let (x, y) = event.position();
                if let Some((x, y)) = session.desktop.source_at(index, x, y) {
                    session.confirm((x, y));
                }
            } else if event.button() == 3 {
                session.finish(None);
            }
            glib::Propagation::Stop
        }));
        confirm.connect_clicked(glib::clone!(@strong session => move |_| {
            let (x, y) = session.selected.get();
            session.confirm((x, y));
        }));
        cancel.connect_clicked(glib::clone!(@strong session => move |_| session.finish(None)));
        window.connect_delete_event(glib::clone!(@strong session => @default-return glib::Propagation::Proceed, move |_, _| {
            if session.done.get() { return glib::Propagation::Proceed; }
            session.finish(None);
            glib::Propagation::Stop
        }));
        window.connect_destroy(glib::clone!(@strong session => move |_| session.finish(None)));
        let bindings = bindings.clone();
        window.connect_key_press_event(glib::clone!(@strong session, @weak canvas => @default-return glib::Propagation::Proceed, move |_, event| {
            use gdk::keys::constants;
            let native_button_key = matches!(event.keyval(), constants::Return | constants::KP_Enter | constants::space);
            if bindings.matches(Action::Cancel, event) && (canvas.has_focus() || !native_button_key) {
                session.finish(None);
                return glib::Propagation::Stop;
            }
            if !canvas.has_focus() { return glib::Propagation::Proceed; }
            let point = session.selected.get();
            if bindings.matches(Action::Confirm, event) {
                session.confirm(point);
                return glib::Propagation::Stop;
            }
            let movement = [(Action::Left, -1, 0), (Action::Right, 1, 0), (Action::Up, 0, -1), (Action::Down, 0, 1),
                (Action::FastLeft, -10, 0), (Action::FastRight, 10, 0), (Action::FastUp, 0, -10), (Action::FastDown, 0, 10)]
                .into_iter().find(|(action, _, _)| bindings.matches(*action, event));
            let Some((_, dx, dy)) = movement else { return glib::Propagation::Proceed; };
            let point = session.desktop.step(point, dx, dy);
            session.update(point);
            session.avoid_controls(point);
            if let Some(target) = session.desktop.monitor_at(point) {
                if target != index { session.focus(target); }
            }
            glib::Propagation::Stop
        }));
        windows.push(window);
    }
    // A changed monitor layout invalidates the frozen desktop's coordinates.
    for id in [
        display
            .connect_monitor_added(glib::clone!(@weak session => move |_, _| session.finish(None))),
        display.connect_monitor_removed(
            glib::clone!(@weak session => move |_, _| session.finish(None)),
        ),
    ] {
        session
            .monitor_signals
            .borrow_mut()
            .push((display.clone().upcast(), id));
    }
    for (index, _) in &monitors {
        if let Some(monitor) = display.monitor(*index) {
            for id in [
                monitor.connect_geometry_notify(
                    glib::clone!(@weak session => move |_| session.finish(None)),
                ),
                monitor.connect_scale_factor_notify(
                    glib::clone!(@weak session => move |_| session.finish(None)),
                ),
            ] {
                session
                    .monitor_signals
                    .borrow_mut()
                    .push((monitor.clone().upcast(), id));
            }
        }
    }
    if session.live.borrow().is_some() {
        *session.refresh.borrow_mut() = Some(glib::timeout_add_local(
            std::time::Duration::from_millis(40),
            glib::clone!(@weak session => @default-return glib::ControlFlow::Break, move || {
                let result = session.live.borrow_mut().as_mut().unwrap().poll();
                match result {
                    Ok(Some(image)) => {
                        session.accept_frame(image);
                    }
                    Ok(None) => (),
                    Err(error) => {
                        eprintln!("Live capture stopped: {error}. Keeping the frozen desktop.");
                        session.refresh.borrow_mut().take();
                        session.live.borrow_mut().take();
                        session.update(session.selected.get());
                        return glib::ControlFlow::Break;
                    }
                }
                glib::ControlFlow::Continue
            }),
        ));
    }
    session.update(selected);
    for window in &windows {
        window.show_all();
    }
    session.focus(active);
    windows[active].clone()
}

fn draw_magnifier(
    cr: &cairo::Context,
    (width, height): (i32, i32),
    pixels: &Pixels,
    point: (i32, i32),
    (px, py): (f64, f64),
    (gx, gy): (f64, f64),
) -> Vec<(f64, f64, f64, f64)> {
    let (ox, oy) = ((gx + 6.0).max(10.0), (gy + 6.0).max(10.0));
    for (value, thickness) in [(0.0, 3.0), (1.0, 1.0)] {
        cr.set_source_rgb(value, value, value);
        cr.set_line_width(thickness);
        cr.move_to(px - ox, py);
        cr.line_to(px - gx, py);
        cr.move_to(px + gx, py);
        cr.line_to(px + ox, py);
        cr.move_to(px, py - oy);
        cr.line_to(px, py - gy);
        cr.move_to(px, py + gy);
        cr.line_to(px, py + oy);
        let _ = cr.stroke();
    }
    // A 15 × 15 source-pixel grid, each pixel rendered as a solid 11px square.
    let box_width = 189.0;
    let box_height = 224.0;
    let bx = if px + box_width + 28.0 < f64::from(width) {
        px + 24.0
    } else {
        px - box_width - 24.0
    }
    .clamp(4.0, (f64::from(width) - box_width - 4.0).max(4.0))
    .round();
    let by = (py + 24.0)
        .min(f64::from(height) - box_height - 8.0)
        .max(4.0)
        .round();
    cr.set_source_rgba(0.06, 0.06, 0.07, 0.98);
    cr.rectangle(bx, by, box_width, box_height);
    let _ = cr.fill();
    cr.set_antialias(cairo::Antialias::None);
    for dy in -7..=7 {
        for dx in -7..=7 {
            let (r, g, b) = pixels.at(point.0 + dx, point.1 + dy).normalized();
            cr.set_source_rgb(r, g, b);
            cr.rectangle(
                bx + 12.0 + f64::from(dx + 7) * 11.0,
                by + 12.0 + f64::from(dy + 7) * 11.0,
                11.0,
                11.0,
            );
            let _ = cr.fill();
        }
    }
    cr.set_antialias(cairo::Antialias::Default);
    cr.set_source_rgb(0.0, 0.0, 0.0);
    cr.set_line_width(3.0);
    cr.rectangle(bx + 89.0, by + 89.0, 11.0, 11.0);
    let _ = cr.stroke();
    cr.set_source_rgb(1.0, 1.0, 1.0);
    cr.set_line_width(1.0);
    cr.rectangle(bx + 89.0, by + 89.0, 11.0, 11.0);
    let _ = cr.stroke();
    cr.select_font_face(
        "monospace",
        cairo::FontSlant::Normal,
        cairo::FontWeight::Bold,
    );
    cr.set_font_size(17.0);
    cr.move_to(bx + 12.0, by + 197.0);
    let _ = cr.show_text(&pixels.at(point.0, point.1).hex());
    cr.set_font_size(11.0);
    cr.move_to(bx + 12.0, by + 215.0);
    let _ = cr.show_text(&format!("x {}   y {}", point.0, point.1));
    vec![
        (bx, by, box_width, box_height),
        (px - ox - 2.0, py - 2.0, ox - gx + 4.0, 4.0),
        (px + gx - 2.0, py - 2.0, ox - gx + 4.0, 4.0),
        (px - 2.0, py - oy - 2.0, 4.0, oy - gy + 4.0),
        (px - 2.0, py + gy - 2.0, 4.0, oy - gy + 4.0),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn settle() {
        let deadline = Instant::now() + Duration::from_millis(80);
        let context = glib::MainContext::default();
        while Instant::now() < deadline {
            while context.pending() {
                context.iteration(false);
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn key(_window: &gtk::ApplicationWindow, key: gdk::keys::Key) {
        // XTEST supplies real focused-window events without moving the pointer
        // (GTK's legacy send-key helper does not handle these CSD windows).
        assert!(std::process::Command::new("xdotool")
            .args(["key", key.name().unwrap().as_str()])
            .status()
            .unwrap()
            .success());
        settle();
    }

    #[test]
    #[ignore = "requires a private native Wayland compositor; run with scripts/check-wayland.sh"]
    fn native_wayland_frozen_overlay() {
        use gstreamer::prelude::*;
        gtk::init().unwrap();
        let display = gdk::Display::default().unwrap();
        assert!(
            display.type_().name().contains("Wayland"),
            "Xvfb cannot verify Wayland fullscreen behavior"
        );
        assert!(std::env::var_os("XDG_CONFIG_HOME").is_some());
        let delivered = Rc::new(Cell::new(false));
        let result = delivered.clone();
        let request = live::start(move |outcome| {
            assert!(matches!(outcome, live::Outcome::Unavailable));
            result.set(true);
        });
        assert!(
            request.is_none(),
            "Unsupported live capture must not request screen sharing"
        );
        assert!(
            delivered.get(),
            "Fallback should be immediate, before opening a portal session"
        );
        let geometry = display.monitor(0).unwrap().geometry();
        let app = gtk::Application::new(
            Some("io.github.opencolorpicker.WaylandTest"),
            gtk::gio::ApplicationFlags::NON_UNIQUE,
        );
        app.register(gtk::gio::Cancellable::NONE).unwrap();
        for confirm in [true, false] {
            let image = Pixbuf::new(
                gtk::gdk_pixbuf::Colorspace::Rgb,
                false,
                8,
                geometry.width(),
                geometry.height(),
            )
            .unwrap();
            image.fill(0x1a2b3cff);
            // Even an already-started stream must become frozen on Wayland.
            let (stream, _, pipeline) = live::test_stream(MonitorArea {
                x: geometry.x(),
                y: geometry.y(),
                width: geometry.width(),
                height: geometry.height(),
            });
            let result = Rc::new(RefCell::new(Vec::new()));
            let delivered = result.clone();
            let window = open_with_stream(
                &app,
                image,
                Bindings::default(),
                Some(stream),
                move |color| delivered.borrow_mut().push(color),
            );
            assert_eq!(pipeline.current_state(), gstreamer::State::Null);
            let overlay = window.child().unwrap().downcast::<gtk::Overlay>().unwrap();
            let canvas = overlay
                .child()
                .unwrap()
                .downcast::<gtk::DrawingArea>()
                .unwrap();
            let deadline = Instant::now() + Duration::from_secs(3);
            while !window
                .window()
                .is_some_and(|w| w.state().contains(gdk::WindowState::FULLSCREEN))
            {
                assert!(
                    Instant::now() < deadline,
                    "Wayland fullscreen configure did not arrive"
                );
                settle();
            }
            settle();
            assert!(!window.is_decorated());
            assert_eq!(
                (canvas.allocated_width(), canvas.allocated_height()),
                (geometry.width(), geometry.height())
            );
            let controls = overlay
                .children()
                .into_iter()
                .find_map(|w| w.downcast::<gtk::Box>().ok())
                .unwrap();
            let hint = controls
                .children()
                .into_iter()
                .find_map(|w| w.downcast::<gtk::Label>().ok())
                .unwrap();
            assert!(hint.text().contains("Frozen desktop"));
            assert!(canvas
                .accessible()
                .unwrap()
                .name()
                .unwrap()
                .contains("#1A2B3C"));
            // Inspect the full Wayland window's output buffer, including alpha.
            // A transparent pixel would expose the compositor's black backdrop.
            let mut surface = cairo::ImageSurface::create(
                cairo::Format::ARgb32,
                geometry.width(),
                geometry.height(),
            )
            .unwrap();
            let cr = cairo::Context::new(&surface).unwrap();
            window.draw(&cr);
            drop(cr);
            surface.flush();
            let stride = surface.stride() as usize;
            {
                let bytes = surface.data().unwrap();
                let offset = 20 * stride + 20 * 4;
                assert_eq!(
                    u32::from_ne_bytes(bytes[offset..offset + 4].try_into().unwrap()),
                    0xff1a2b3c,
                    "Frozen desktop must be opaque and preserve the captured color"
                );
            }
            let pixels =
                gdk::pixbuf_get_from_surface(&surface, 0, 0, geometry.width(), geometry.height())
                    .unwrap();
            let rendered = Pixels::new(&pixels);
            let magnifier_pixels = (0..geometry.height() - 120)
                .flat_map(|y| (0..geometry.width()).map(move |x| (x, y)))
                .filter(|&(x, y)| rendered.at(x, y) != Color(26, 43, 60))
                .count();
            assert!(
                magnifier_pixels > 1000,
                "Magnifier must remain visible above the controls"
            );
            // Capture Weston's actual output from its private Xvfb host. This
            // catches compositor black fill that a widget-only render misses.
            let mut output = live::test_desktop_stream(MonitorArea {
                x: 0,
                y: 0,
                width: geometry.width(),
                height: geometry.height(),
            });
            let deadline = Instant::now() + Duration::from_secs(3);
            let composited = loop {
                if let Some(image) = output.poll().unwrap() {
                    if Pixels::new(&image).at(20, 20) == Color(26, 43, 60) {
                        break image;
                    }
                }
                assert!(
                    Instant::now() < deadline,
                    "Wayland compositor output does not show the captured desktop"
                );
                settle();
            };
            drop(output);
            if let Some(directory) = std::env::var_os("OCP_UI_CAPTURE_DIR") {
                std::fs::create_dir_all(&directory).unwrap();
                let theme = std::env::var("OCP_UI_THEME_LABEL").unwrap_or_else(|_| "light".into());
                composited
                    .savev(
                        std::path::Path::new(&directory)
                            .join(format!("wayland-frozen-{theme}.png")),
                        "png",
                        &[],
                    )
                    .unwrap();
            }
            let target = if confirm {
                "picker-confirm"
            } else {
                "picker-cancel"
            };
            let button = controls
                .children()
                .into_iter()
                .find(|w| w.widget_name() == target)
                .unwrap()
                .downcast::<gtk::Button>()
                .unwrap();
            button.emit_clicked();
            settle();
            assert_eq!(
                *result.borrow(),
                vec![if confirm {
                    Some(Color(26, 43, 60))
                } else {
                    None
                }]
            );
        }
    }

    #[test]
    #[ignore = "requires private Xvfb/D-Bus and xcompmgr; run with scripts/check.sh"]
    fn live_desktop_stays_visible_without_sampling_overlay() {
        gtk::init().unwrap();
        let screen = gdk::Screen::default().unwrap();
        assert!(
            !screen.is_composited(),
            "Run only in a private Xvfb display"
        );
        let mut compositor = std::process::Command::new("xcompmgr")
            .arg("-n")
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while !screen.is_composited() {
            assert!(Instant::now() < deadline, "Compositor did not start");
            settle();
        }
        let geometry = gdk::Display::default()
            .unwrap()
            .monitor(0)
            .unwrap()
            .geometry();
        let backdrop = gtk::Window::new(gtk::WindowType::Toplevel);
        backdrop.set_decorated(false);
        backdrop.set_default_size(geometry.width(), geometry.height());
        backdrop.move_(0, 0);
        let background = Rc::new(Cell::new(Color(26, 43, 60)));
        backdrop.connect_draw(glib::clone!(@strong background => move |_, cr| {
            let (r, g, b) = background.get().normalized();
            cr.set_source_rgb(r, g, b);
            cr.paint().unwrap();
            glib::Propagation::Stop
        }));
        backdrop.show_all();
        settle();
        let root_color = |color| {
            background.set(color);
            backdrop.queue_draw();
        };
        let pointer = |x: i32, y: i32| {
            assert!(std::process::Command::new("xdotool")
                .args(["mousemove", &x.to_string(), &y.to_string()])
                .status()
                .unwrap()
                .success())
        };
        pointer(geometry.width() / 2, geometry.height() / 2);
        let mut stream = live::test_desktop_stream(MonitorArea {
            x: 0,
            y: 0,
            width: geometry.width(),
            height: geometry.height(),
        });
        let deadline = Instant::now() + Duration::from_secs(3);
        let image = loop {
            if let Some(image) = stream.poll().unwrap() {
                break image;
            }
            assert!(Instant::now() < deadline);
            settle();
        };
        assert_eq!(Pixels::new(&image).at(20, 20), Color(26, 43, 60));
        let app = gtk::Application::new(
            Some("io.github.opencolorpicker.FeedbackTest"),
            gtk::gio::ApplicationFlags::NON_UNIQUE,
        );
        app.register(gtk::gio::Cancellable::NONE).unwrap();
        let result = Rc::new(RefCell::new(Vec::new()));
        let delivered = result.clone();
        let window = open_with_stream(
            &app,
            image,
            Bindings::default(),
            Some(stream),
            move |color| delivered.borrow_mut().push(color),
        );
        let canvas = window
            .child()
            .unwrap()
            .downcast::<gtk::Overlay>()
            .unwrap()
            .child()
            .unwrap()
            .downcast::<gtk::DrawingArea>()
            .unwrap();
        window.window().unwrap().focus(0);
        canvas.grab_focus();
        let expect_color = |hex: &str| {
            let deadline = Instant::now() + Duration::from_secs(3);
            while !canvas.accessible().unwrap().name().unwrap().contains(hex) {
                assert!(
                    Instant::now() < deadline,
                    "Live selection did not become {hex}"
                );
                settle();
            }
            for _ in 0..5 {
                settle();
                assert!(
                    canvas.accessible().unwrap().name().unwrap().contains(hex),
                    "Overlay fed back into sampling"
                );
            }
        };
        expect_color("#1A2B3C");
        root_color(Color(68, 136, 204));
        expect_color("#4488CC");
        let screen_image = gdk::Window::default_root_window()
            .pixbuf(0, 0, geometry.width(), geometry.height())
            .unwrap();
        assert_eq!(
            Pixels::new(&screen_image).at(20, 20),
            Color(68, 136, 204),
            "The actual composited desktop must remain visible"
        );
        if let Some(directory) = std::env::var_os("OCP_UI_CAPTURE_DIR") {
            std::fs::create_dir_all(&directory).unwrap();
            let theme = std::env::var("OCP_UI_THEME_LABEL").unwrap_or_else(|_| "light".into());
            screen_image
                .savev(
                    std::path::Path::new(&directory).join(format!("live-desktop-{theme}.png")),
                    "png",
                    &[],
                )
                .unwrap();
        }
        // Move into the previous magnifier, then change the desktop again.
        pointer(geometry.width() / 2 + 75, geometry.height() / 2 + 75);
        root_color(Color(204, 136, 68));
        expect_color("#CC8844");
        key(&window, gdk::keys::constants::Return);
        let deadline = Instant::now() + Duration::from_secs(2);
        while result.borrow().is_empty() {
            assert!(Instant::now() < deadline);
            settle();
        }
        assert_eq!(*result.borrow(), vec![Some(Color(204, 136, 68))]);
        compositor.kill().unwrap();
        compositor.wait().unwrap();
        backdrop.close();
    }

    #[test]
    #[ignore = "requires private Xvfb/D-Bus; run with scripts/check.sh"]
    fn live_overlay_updates_then_freezes_with_magnifier() {
        use gstreamer::prelude::*;
        gtk::init().unwrap();
        let geometry = gdk::Display::default()
            .unwrap()
            .monitor(0)
            .unwrap()
            .geometry();
        assert!(std::process::Command::new("xdotool")
            .args([
                "mousemove",
                &(geometry.width() / 2).to_string(),
                &(geometry.height() / 2).to_string()
            ])
            .status()
            .unwrap()
            .success());
        let (mut stream, source, pipeline) = live::test_stream(MonitorArea {
            x: geometry.x(),
            y: geometry.y(),
            width: geometry.width(),
            height: geometry.height(),
        });
        let deadline = Instant::now() + Duration::from_secs(3);
        let image = loop {
            if let Some(image) = stream.poll().unwrap() {
                break image;
            }
            assert!(Instant::now() < deadline);
            settle();
        };
        let app = gtk::Application::new(
            Some("io.github.opencolorpicker.LiveTest"),
            gtk::gio::ApplicationFlags::NON_UNIQUE,
        );
        app.register(gtk::gio::Cancellable::NONE).unwrap();
        let result = Rc::new(RefCell::new(Vec::new()));
        let delivered = result.clone();
        let window = open_with_stream(
            &app,
            image,
            Bindings::default(),
            Some(stream),
            move |color| delivered.borrow_mut().push(color),
        );
        let overlay = window.child().unwrap().downcast::<gtk::Overlay>().unwrap();
        let canvas = overlay
            .child()
            .unwrap()
            .downcast::<gtk::DrawingArea>()
            .unwrap();
        let controls = overlay
            .children()
            .into_iter()
            .find_map(|w| w.downcast::<gtk::Box>().ok())
            .unwrap();
        let hint = controls
            .children()
            .into_iter()
            .find_map(|w| w.downcast::<gtk::Label>().ok())
            .unwrap();
        window.window().unwrap().focus(0);
        canvas.grab_focus();
        settle();
        assert!(hint.text().contains("Live desktop"));
        let sample = |x: usize, y: usize| {
            let mut surface = cairo::ImageSurface::create(
                cairo::Format::ARgb32,
                geometry.width(),
                geometry.height(),
            )
            .unwrap();
            let cr = cairo::Context::new(&surface).unwrap();
            canvas.draw(&cr);
            drop(cr);
            surface.flush();
            let stride = surface.stride() as usize;
            let data = surface.data().unwrap();
            u32::from_ne_bytes(
                data[y * stride + x * 4..y * stride + x * 4 + 4]
                    .try_into()
                    .unwrap(),
            )
        };
        assert_eq!(
            sample(0, 0),
            0,
            "Live picking must leave the desktop visible"
        );
        source.set_property_from_str("pattern", "green");
        let deadline = Instant::now() + Duration::from_secs(3);
        while !canvas
            .accessible()
            .unwrap()
            .name()
            .unwrap()
            .contains("#00FF00")
        {
            assert!(
                Instant::now() < deadline,
                "Magnifier did not follow live frames"
            );
            settle();
        }
        assert!(pipeline.send_event(gstreamer::event::Eos::new()));
        let deadline = Instant::now() + Duration::from_secs(3);
        while !hint.text().contains("Frozen desktop") {
            assert!(
                Instant::now() < deadline,
                "Frozen fallback did not activate"
            );
            settle();
        }
        assert_eq!(pipeline.current_state(), gstreamer::State::Null);
        assert_eq!(
            sample(0, 0),
            0xff00ff00,
            "Frozen fallback must show the last desktop frame"
        );
        assert!(!window.is_decorated());
        assert_eq!(canvas.allocated_width(), geometry.width());
        assert_eq!(canvas.allocated_height(), geometry.height());
        key(&window, gdk::keys::constants::Right);
        key(&window, gdk::keys::constants::Return);
        assert_eq!(*result.borrow(), vec![Some(Color(0, 255, 0))]);
    }

    #[test]
    #[ignore = "requires a private X11 display; run with xvfb-run and dbus-run-session"]
    fn picker_keyboard_and_accessibility() {
        gtk::init().unwrap();
        let geometry = gdk::Display::default()
            .unwrap()
            .monitor(0)
            .unwrap()
            .geometry();
        assert!(std::process::Command::new("xdotool")
            .args([
                "mousemove",
                &(geometry.width() / 2).to_string(),
                &(geometry.height() / 2).to_string()
            ])
            .status()
            .unwrap()
            .success());
        let app = gtk::Application::new(
            Some("io.github.opencolorpicker.KeyboardTest"),
            gtk::gio::ApplicationFlags::NON_UNIQUE,
        );
        app.register(gtk::gio::Cancellable::NONE).unwrap();
        for target in ["cancel", "confirm", "canvas"] {
            let image = Pixbuf::new(gtk::gdk_pixbuf::Colorspace::Rgb, false, 8, 8, 4).unwrap();
            image.fill(0x1a2b3cff);
            let result = Rc::new(RefCell::new(None));
            let delivered = result.clone();
            let window = open(&app, image, Bindings::default(), move |color| {
                *delivered.borrow_mut() = Some(color)
            });
            let overlay = window.child().unwrap().downcast::<gtk::Overlay>().unwrap();
            let canvas = overlay
                .child()
                .unwrap()
                .downcast::<gtk::DrawingArea>()
                .unwrap();
            assert!(!window.is_decorated());
            assert!(window.titlebar().is_none());
            let controls = overlay
                .children()
                .into_iter()
                .find_map(|child| child.downcast::<gtk::Box>().ok())
                .unwrap();
            let button = |label: &str| {
                controls
                    .children()
                    .into_iter()
                    .filter_map(|w| w.downcast::<gtk::Button>().ok())
                    .find(|b| b.label().as_deref() == Some(label))
                    .unwrap()
            };
            let cancel = button("Cancel");
            let confirm = button("Use color");
            window.window().unwrap().focus(0);
            canvas.grab_focus();
            settle();
            assert_eq!(canvas.allocated_width(), geometry.width());
            assert_eq!(canvas.allocated_height(), geometry.height());
            assert!(canvas.has_focus());
            assert!(canvas.accessible().unwrap().name().unwrap().contains("x 4"));
            key(&window, gdk::keys::constants::Right);
            assert!(canvas.accessible().unwrap().name().unwrap().contains("x 5"));
            assert!(confirm
                .accessible()
                .unwrap()
                .description()
                .unwrap()
                .contains("x 5"));
            match target {
                "cancel" => {
                    for _ in 0..12 {
                        key(&window, gdk::keys::constants::Tab);
                        if cancel.has_focus() {
                            break;
                        }
                    }
                    assert!(cancel.has_focus(), "Tab must reach Cancel");
                }
                "confirm" => {
                    confirm.grab_focus();
                    settle();
                }
                _ => (),
            }
            key(&window, gdk::keys::constants::Return);
            let deadline = Instant::now() + Duration::from_secs(2);
            while result.borrow().is_none() && Instant::now() < deadline {
                settle();
            }
            assert_eq!(
                *result.borrow(),
                Some(if target == "cancel" {
                    None
                } else {
                    Some(Color(26, 43, 60))
                }),
                "focused {target}"
            );
        }
        // Remapping applies to real picker events, removes old bindings and
        // updates the accessible instructions, including disabled controls.
        let mut bindings = Bindings::default();
        bindings.set(Action::Right, "d").unwrap();
        bindings.set(Action::FastRight, "<Shift>d").unwrap();
        bindings.set(Action::Confirm, "space").unwrap();
        bindings.set(Action::Cancel, "q").unwrap();
        bindings.set(Action::Left, "").unwrap();
        let image = Pixbuf::new(gtk::gdk_pixbuf::Colorspace::Rgb, false, 8, 80, 4).unwrap();
        image.fill(0x1a2b3cff);
        let result = Rc::new(RefCell::new(None));
        let delivered = result.clone();
        let window = open(&app, image, bindings, move |color| {
            *delivered.borrow_mut() = Some(color)
        });
        let canvas = window
            .child()
            .unwrap()
            .downcast::<gtk::Overlay>()
            .unwrap()
            .child()
            .unwrap()
            .downcast::<gtk::DrawingArea>()
            .unwrap();
        window.window().unwrap().focus(0);
        canvas.grab_focus();
        settle();
        key(&window, gdk::keys::constants::Right);
        key(&window, gdk::keys::constants::Left);
        assert!(canvas
            .accessible()
            .unwrap()
            .name()
            .unwrap()
            .contains("x 40"));
        key(&window, gdk::keys::constants::d);
        assert!(canvas
            .accessible()
            .unwrap()
            .name()
            .unwrap()
            .contains("x 41"));
        assert!(std::process::Command::new("xdotool")
            .args(["key", "shift+d"])
            .status()
            .unwrap()
            .success());
        settle();
        assert!(canvas
            .accessible()
            .unwrap()
            .name()
            .unwrap()
            .contains("x 51"));
        assert!(canvas
            .accessible()
            .unwrap()
            .description()
            .unwrap()
            .contains("Disabled"));
        key(&window, gdk::keys::constants::Return);
        assert!(result.borrow().is_none());
        key(&window, gdk::keys::constants::space);
        assert_eq!(*result.borrow(), Some(Some(Color(26, 43, 60))));
    }
}
