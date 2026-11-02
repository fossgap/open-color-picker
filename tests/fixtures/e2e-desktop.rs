//! A controlled desktop and pixel probe for the external, unmodified app E2E test.
use gtk::{gdk, glib, prelude::*};
use std::{cell::Cell, rc::Rc, time::Duration};

fn main() {
    gtk::init().expect("private X11 display");
    assert!(gdk::Display::default()
        .unwrap()
        .type_()
        .name()
        .contains("X11"));
    let args: Vec<_> = std::env::args().collect();
    match args[1].as_str() {
        "background" => {
            let path = args[2].clone();
            let color = Rc::new(Cell::new(0_u32));
            let window = gtk::Window::new(gtk::WindowType::Toplevel);
            window.set_title("E2E controlled desktop");
            window.set_type_hint(gdk::WindowTypeHint::Desktop);
            window.set_decorated(false);
            let monitor = gdk::Display::default()
                .unwrap()
                .monitor(0)
                .unwrap()
                .geometry();
            window.move_(monitor.x(), monitor.y());
            window.set_default_size(monitor.width(), monitor.height());
            window.connect_draw(glib::clone!(@strong color => move |_, cr| {
                let rgb = color.get();
                cr.set_source_rgb(
                    f64::from((rgb >> 16) & 255) / 255.0,
                    f64::from((rgb >> 8) & 255) / 255.0,
                    f64::from(rgb & 255) / 255.0,
                );
                cr.paint().unwrap();
                glib::Propagation::Stop
            }));
            glib::timeout_add_local(
                Duration::from_millis(40),
                glib::clone!(@weak window => @default-return glib::ControlFlow::Break, move || {
                    if let Ok(text) = std::fs::read_to_string(&path) {
                        if let Ok(rgb) = u32::from_str_radix(text.trim().trim_start_matches('#'), 16) {
                            if color.replace(rgb) != rgb { window.queue_draw(); }
                        }
                    }
                    glib::ControlFlow::Continue
                }),
            );
            window.show_all();
            gtk::main();
        }
        "pixel" | "snapshot" => {
            let root = gdk::Window::default_root_window();
            let (_, _, width, height) = root.geometry();
            let image = root.pixbuf(0, 0, width, height).unwrap();
            if args[1] == "snapshot" {
                image.savev(&args[2], "png", &[]).unwrap();
            } else {
                let x: usize = args[2].parse().unwrap();
                let y: usize = args[3].parse().unwrap();
                assert!(x < width as usize && y < height as usize);
                let bytes = image.read_pixel_bytes();
                let offset = y * image.rowstride() as usize + x * image.n_channels() as usize;
                println!(
                    "#{:02X}{:02X}{:02X}",
                    bytes[offset],
                    bytes[offset + 1],
                    bytes[offset + 2]
                );
            }
        }
        _ => panic!("expected background COLOR_FILE, pixel X Y, or snapshot PNG"),
    }
}
