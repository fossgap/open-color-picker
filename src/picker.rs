use crate::{
    color::Color,
    pixels::{self, Pixels},
    shortcuts::{Action, Bindings},
};
use gtk::{atk::prelude::*, cairo, gdk, gdk_pixbuf::Pixbuf, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

type PickCallback = Rc<RefCell<Option<Box<dyn FnOnce(Option<Color>)>>>>;

pub fn open(
    app: &gtk::Application,
    image: Pixbuf,
    bindings: Bindings,
    on_pick: impl FnOnce(Option<Color>) + 'static,
) -> gtk::ApplicationWindow {
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("Pick a pixel — Open Color Picker")
        .default_width(1000)
        .default_height(700)
        .build();
    let header = gtk::HeaderBar::builder()
        .title("Pick a pixel")
        .subtitle("Captured screen · original pixel values")
        .show_close_button(true)
        .build();
    let cancel = gtk::Button::with_label("Cancel");
    let confirm = gtk::Button::with_label("Use color");
    confirm.style_context().add_class("suggested-action");
    header.pack_start(&cancel);
    header.pack_end(&confirm);
    window.set_titlebar(Some(&header));
    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let canvas = gtk::DrawingArea::new();
    canvas.set_can_focus(true);
    canvas.set_hexpand(true);
    canvas.set_vexpand(true);
    canvas.add_events(gdk::EventMask::POINTER_MOTION_MASK | gdk::EventMask::BUTTON_PRESS_MASK);
    let help_text = bindings.picker_help();
    let help = gtk::Label::new(Some(&help_text));
    help.set_line_wrap(true);
    help.set_margin_top(12);
    help.set_margin_bottom(12);
    help.set_margin_start(16);
    help.set_margin_end(16);
    let readout = gtk::Label::new(None);
    readout.set_margin_top(8);
    column.pack_start(&canvas, true, true, 0);
    column.pack_start(&readout, false, false, 0);
    column.pack_start(&help, false, false, 0);
    window.add(&column);
    let pixels = Rc::new(Pixels::new(&image));
    let selected = Rc::new(Cell::new((pixels.width / 2, pixels.height / 2)));
    let callback: PickCallback = Rc::new(RefCell::new(Some(Box::new(on_pick))));
    update_readout(&canvas, &confirm, &readout, &pixels, selected.get());
    if let Some(accessible) = canvas.accessible() {
        accessible.set_description(&help_text);
    }

    canvas.connect_draw(glib::clone!(@strong pixels, @strong selected => @default-return glib::Propagation::Proceed, move |area, cr| {
        draw(cr, area.allocated_width(), area.allocated_height(), &image, &pixels, selected.get());
        glib::Propagation::Stop
    }));
    canvas.connect_motion_notify_event(glib::clone!(@strong pixels, @strong selected, @weak confirm, @weak readout => @default-return glib::Propagation::Proceed, move |area, event| {
        let (x,y) = event.position();
        if let Some(point) = pixels::position(x,y,(pixels.width,pixels.height),(area.allocated_width(),area.allocated_height())) {
            selected.set(point);
            update_readout(area, &confirm, &readout, &pixels, point);
            area.queue_draw();
        }
        glib::Propagation::Stop
    }));
    canvas.connect_button_press_event(glib::clone!(@strong pixels, @strong callback, @weak window => @default-return glib::Propagation::Proceed, move |area,event| {
        if event.button() == 1 {
            let (x,y) = event.position();
            if let Some((x,y)) = pixels::position(x,y,(pixels.width,pixels.height),(area.allocated_width(),area.allocated_height())) {
                finish(&window, &callback, Some(pixels.at(x,y)));
            }
        }
        glib::Propagation::Stop
    }));
    confirm.connect_clicked(
        glib::clone!(@strong pixels, @strong selected, @strong callback, @weak window => move |_| {
            let (x,y) = selected.get(); finish(&window,&callback,Some(pixels.at(x,y)));
        }),
    );
    cancel.connect_clicked(
        glib::clone!(@strong callback, @weak window => move |_| finish(&window,&callback,None)),
    );
    window.connect_delete_event(
        glib::clone!(@strong callback => @default-return glib::Propagation::Proceed, move |_,_| {
            let callback = callback.borrow_mut().take();
            if let Some(callback) = callback { callback(None); }
            glib::Propagation::Proceed
        }),
    );
    window.connect_key_press_event(glib::clone!(@strong pixels, @strong selected, @strong callback, @weak canvas, @weak confirm, @weak readout => @default-return glib::Propagation::Proceed, move |window,event| {
        use gdk::keys::constants;
        let native_button_key = matches!(event.keyval(), constants::Return | constants::KP_Enter | constants::space);
        if bindings.matches(Action::Cancel, event) && (canvas.has_focus() || !native_button_key) {
            finish(window,&callback,None);
            return glib::Propagation::Stop;
        }
        // Buttons retain their native Enter/Space activation and focus navigation.
        if !canvas.has_focus() { return glib::Propagation::Proceed; }
        let (mut x,mut y) = selected.get();
        if bindings.matches(Action::Confirm, event) {
            finish(window,&callback,Some(pixels.at(x,y)));
            return glib::Propagation::Stop;
        }
        let movement = [(Action::Left, -1, 0), (Action::Right, 1, 0), (Action::Up, 0, -1), (Action::Down, 0, 1),
            (Action::FastLeft, -10, 0), (Action::FastRight, 10, 0), (Action::FastUp, 0, -10), (Action::FastDown, 0, 10)]
            .into_iter().find(|(action, _, _)| bindings.matches(*action, event));
        let Some((_, dx, dy)) = movement else { return glib::Propagation::Proceed; };
        x += dx; y += dy;
        selected.set((x.clamp(0,pixels.width-1),y.clamp(0,pixels.height-1)));
        update_readout(&canvas, &confirm, &readout, &pixels, selected.get());
        canvas.queue_draw();
        glib::Propagation::Stop
    }));
    window.maximize();
    window.show_all();
    canvas.grab_focus();
    window.present();
    window
}

fn update_readout(
    canvas: &gtk::DrawingArea,
    confirm: &gtk::Button,
    readout: &gtk::Label,
    pixels: &Pixels,
    point: (i32, i32),
) {
    let description = format!(
        "{} · x {} · y {}",
        pixels.at(point.0, point.1),
        point.0,
        point.1
    );
    readout.set_text(&description);
    if let Some(accessible) = canvas.accessible() {
        accessible.set_name(&format!("Captured image: {description}"));
    }
    confirm.set_tooltip_text(Some(&format!("Use {description}")));
    if let Some(accessible) = confirm.accessible() {
        accessible.set_description(&description);
    }
}

fn finish(window: &gtk::ApplicationWindow, callback: &PickCallback, color: Option<Color>) {
    let callback = callback.borrow_mut().take();
    window.hide();
    window.close();
    if let Some(callback) = callback {
        callback(color);
    }
}

fn draw(
    cr: &cairo::Context,
    width: i32,
    height: i32,
    image: &Pixbuf,
    pixels: &Pixels,
    point: (i32, i32),
) {
    let (scale, left, top) = pixels::fit((pixels.width, pixels.height), (width, height));
    cr.set_source_rgb(0.08, 0.08, 0.09);
    let _ = cr.paint();
    let _ = cr.save();
    cr.translate(left, top);
    cr.scale(scale, scale);
    cr.set_source_pixbuf(image, 0.0, 0.0);
    cr.source().set_filter(cairo::Filter::Nearest);
    let _ = cr.paint();
    let _ = cr.restore();
    let (px, py) = (
        left + (f64::from(point.0) + 0.5) * scale,
        top + (f64::from(point.1) + 0.5) * scale,
    );
    cr.set_source_rgb(0.0, 0.0, 0.0);
    cr.set_line_width(3.0);
    cr.move_to(px - 10.0, py);
    cr.line_to(px + 10.0, py);
    cr.move_to(px, py - 10.0);
    cr.line_to(px, py + 10.0);
    let _ = cr.stroke();
    cr.set_source_rgb(1.0, 1.0, 1.0);
    cr.set_line_width(1.0);
    cr.move_to(px - 10.0, py);
    cr.line_to(px + 10.0, py);
    cr.move_to(px, py - 10.0);
    cr.line_to(px, py + 10.0);
    let _ = cr.stroke();
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
    #[ignore = "requires a private X11 display; run with xvfb-run and dbus-run-session"]
    fn picker_keyboard_and_accessibility() {
        gtk::init().unwrap();
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
            let column = window.child().unwrap().downcast::<gtk::Box>().unwrap();
            let canvas = column.children()[0]
                .clone()
                .downcast::<gtk::DrawingArea>()
                .unwrap();
            let header = window
                .titlebar()
                .unwrap()
                .downcast::<gtk::HeaderBar>()
                .unwrap();
            let button = |label: &str| {
                header
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
            .downcast::<gtk::Box>()
            .unwrap()
            .children()[0]
            .clone()
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
