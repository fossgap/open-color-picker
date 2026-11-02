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

pub fn fit(image: (i32, i32), view: (i32, i32)) -> (f64, f64, f64) {
    let scale =
        (f64::from(view.0) / f64::from(image.0)).min(f64::from(view.1) / f64::from(image.1));
    (
        scale,
        (f64::from(view.0) - f64::from(image.0) * scale) / 2.0,
        (f64::from(view.1) - f64::from(image.1) * scale) / 2.0,
    )
}

pub fn position(x: f64, y: f64, image: (i32, i32), view: (i32, i32)) -> Option<(i32, i32)> {
    let (scale, left, top) = fit(image, view);
    if scale <= 0.0 {
        return None;
    }
    let (x, y) = ((x - left) / scale, (y - top) / scale);
    if x >= 0.0 && y >= 0.0 && x < f64::from(image.0) && y < f64::from(image.1) {
        Some((x.floor() as i32, y.floor() as i32))
    } else {
        None
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
    fn maps_letterboxing_and_hidpi_to_source_pixels() {
        assert_eq!(position(0.0, 0.0, (400, 200), (200, 200)), None);
        assert_eq!(
            position(100.0, 100.0, (400, 200), (200, 200)),
            Some((200, 100))
        );
        assert_eq!(
            position(199.9, 149.9, (400, 200), (200, 200)),
            Some((399, 199))
        );
        assert_eq!(position(200.0, 150.0, (400, 200), (200, 200)), None);
    }
}
