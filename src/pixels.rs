use crate::color::Color;
use gtk::gdk_pixbuf::Pixbuf;

pub struct Pixels {
    pub width: i32,
    pub height: i32,
    stride: usize,
    channels: usize,
    bytes: gtk::glib::Bytes,
}

impl Pixels {
    pub fn new(image: &Pixbuf) -> Self {
        Self {
            width: image.width(),
            height: image.height(),
            stride: image.rowstride() as usize,
            channels: image.n_channels() as usize,
            bytes: image.read_pixel_bytes(),
        }
    }
    pub fn at(&self, x: i32, y: i32) -> Color {
        let x = x.clamp(0, self.width - 1) as usize;
        let y = y.clamp(0, self.height - 1) as usize;
        let i = y * self.stride + x * self.channels;
        Color(self.bytes[i], self.bytes[i + 1], self.bytes[i + 2])
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MonitorArea {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// The capture covers the desktop's bounding rectangle. Monitor geometry is
/// logical; the capture can contain more pixels on HiDPI/fractional-scale desktops.
pub struct Desktop {
    pub monitors: Vec<MonitorArea>,
    pub origin: (i32, i32),
    pub scale: (f64, f64),
    image: (i32, i32),
}

impl Desktop {
    pub fn new(image: (i32, i32), monitors: Vec<MonitorArea>) -> Self {
        assert!(!monitors.is_empty());
        let left = monitors.iter().map(|m| m.x).min().unwrap();
        let top = monitors.iter().map(|m| m.y).min().unwrap();
        let right = monitors.iter().map(|m| m.x + m.width).max().unwrap();
        let bottom = monitors.iter().map(|m| m.y + m.height).max().unwrap();
        Self {
            monitors,
            origin: (left, top),
            scale: (
                f64::from(image.0) / f64::from(right - left),
                f64::from(image.1) / f64::from(bottom - top),
            ),
            image,
        }
    }

    pub fn source_at(&self, monitor: usize, x: f64, y: f64) -> Option<(i32, i32)> {
        let m = self.monitors[monitor];
        if !(0.0..f64::from(m.width)).contains(&x) || !(0.0..f64::from(m.height)).contains(&y) {
            return None;
        }
        Some((
            ((f64::from(m.x - self.origin.0) + x) * self.scale.0).floor() as i32,
            ((f64::from(m.y - self.origin.1) + y) * self.scale.1).floor() as i32,
        ))
    }

    pub fn local_at(&self, monitor: usize, point: (i32, i32)) -> (f64, f64) {
        let m = self.monitors[monitor];
        (
            (f64::from(point.0) + 0.5) / self.scale.0 - f64::from(m.x - self.origin.0),
            (f64::from(point.1) + 0.5) / self.scale.1 - f64::from(m.y - self.origin.1),
        )
    }

    pub fn monitor_at(&self, point: (i32, i32)) -> Option<usize> {
        self.monitors.iter().enumerate().find_map(|(i, m)| {
            let (x, y) = self.local_at(i, point);
            ((0.0..f64::from(m.width)).contains(&x) && (0.0..f64::from(m.height)).contains(&y))
                .then_some(i)
        })
    }

    pub fn step(&self, point: (i32, i32), dx: i32, dy: i32) -> (i32, i32) {
        let mut current = point;
        for _ in 0..dx.abs().max(dy.abs()) {
            let mut candidate = current;
            // Skip monitor gaps and stop at the last visible pixel at an edge,
            // including when a ten-pixel movement would overshoot it.
            loop {
                candidate.0 += dx.signum();
                candidate.1 += dy.signum();
                if !(0..self.image.0).contains(&candidate.0)
                    || !(0..self.image.1).contains(&candidate.1)
                {
                    return current;
                }
                if self.monitor_at(candidate).is_some() {
                    current = candidate;
                    break;
                }
            }
        }
        current
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn samples_padding_alpha_and_edges() {
        let bytes =
            gtk::glib::Bytes::from_owned(vec![1, 2, 3, 4, 5, 6, 99, 99, 7, 8, 9, 10, 11, 12]);
        let pixels = Pixels {
            width: 2,
            height: 2,
            stride: 8,
            channels: 3,
            bytes,
        };
        assert_eq!(pixels.at(0, 1), Color(7, 8, 9));
        assert_eq!(pixels.at(100, 100), Color(10, 11, 12));
        let rgba = Pixels {
            width: 1,
            height: 1,
            stride: 4,
            channels: 4,
            bytes: gtk::glib::Bytes::from_owned(vec![3, 4, 5, 255]),
        };
        assert_eq!(rgba.at(0, 0), Color(3, 4, 5));
    }
    #[test]
    fn maps_desktop_coordinates_without_letterboxing() {
        let desktop = Desktop::new(
            (3840, 2160),
            vec![MonitorArea {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            }],
        );
        assert_eq!(desktop.source_at(0, 0.0, 0.0), Some((0, 0)));
        assert_eq!(desktop.source_at(0, 960.0, 540.0), Some((1920, 1080)));
        assert_eq!(desktop.source_at(0, 1919.9, 1079.9), Some((3839, 2159)));
        assert_eq!(desktop.source_at(0, 1920.0, 1080.0), None);
        assert_eq!(desktop.local_at(0, (1920, 1080)), (960.25, 540.25));
    }

    #[test]
    fn maps_negative_monitor_origins_fractional_scaling_and_gaps() {
        let desktop = Desktop::new(
            (4200, 1800),
            vec![
                MonitorArea {
                    x: -1600,
                    y: 0,
                    width: 1600,
                    height: 900,
                },
                MonitorArea {
                    x: 0,
                    y: 300,
                    width: 1200,
                    height: 900,
                },
            ],
        );
        assert_eq!(desktop.source_at(1, 0.0, 0.0), Some((2400, 450)));
        assert_eq!(desktop.source_at(1, 100.0, 100.0), Some((2550, 600)));
        assert_eq!(desktop.monitor_at((2400, 450)), Some(1));
        assert_eq!(desktop.monitor_at((2400, 449)), None);
        assert_eq!(desktop.step((2399, 600), 1, 0), (2400, 600));
        assert_eq!(desktop.step((2399, 100), 1, 0), (2399, 100));
        assert_eq!(desktop.step((2395, 100), 10, 0), (2399, 100));
        assert_eq!(desktop.step((0, 0), -1, 0), (0, 0));
    }
}
