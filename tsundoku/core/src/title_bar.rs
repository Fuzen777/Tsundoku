//! The main window's own title bar (the app crate's `title_bar`): where the UI drew its maximize button, which a window
//! of the app's lies over so that Windows shows its Snap Layouts there, and what the pointer does on that button, which
//! the UI can't see under it.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Where the UI drew its maximize button, in CSS pixels from the window's top right corner, so the window over it
/// follows the window's width and DPI without asking the UI again.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ButtonPlace {
    /// From the window's right edge to the button's.
    pub right: f64,
    /// From the window's top edge to the button's.
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

/// A rectangle in a window's client area, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl ButtonPlace {
    /// The button in a client area `client_width` physical pixels wide, at `scale` physical pixels per CSS pixel
    /// (1.0 at 96 DPI), less its top `top_border` pixels, which stay the resize border along the window's top. Each
    /// edge is rounded on its own, so the buttons beside it meet it without a gap.
    pub fn in_pixels(&self, client_width: i32, scale: f64, top_border: i32) -> PixelRect {
        let pixels = |css: f64| (css * scale).round() as i32;
        let left = client_width - pixels(self.right + self.width);
        let right = client_width - pixels(self.right);
        let top = pixels(self.top).max(top_border);
        let bottom = pixels(self.top + self.height);
        PixelRect {
            x: left,
            y: top,
            width: (right - left).max(0),
            height: (bottom - top).max(0),
        }
    }
}

/// What the pointer does on the maximize button, for the UI to show as it shows the other caption buttons' hover and
/// press.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum MaximizeButtonState {
    /// The pointer is elsewhere.
    Rest,
    /// The pointer is over it.
    Hovered,
    /// It's pressed, and maximizes or restores the window once released over it.
    Pressed,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The maximize button as the UI draws it: 46 px wide, beside Close, the title bar's 56 px high.
    const BESIDE_CLOSE: ButtonPlace = ButtonPlace {
        right: 46.0,
        top: 0.0,
        width: 46.0,
        height: 56.0,
    };

    #[test]
    fn places_the_button_from_the_top_right_corner() {
        assert_eq!(
            BESIDE_CLOSE.in_pixels(1100, 1.0, 0),
            PixelRect {
                x: 1008,
                y: 0,
                width: 46,
                height: 56
            }
        );
    }

    #[test]
    fn scales_to_physical_pixels() {
        assert_eq!(
            BESIDE_CLOSE.in_pixels(1650, 1.5, 0),
            PixelRect {
                x: 1512,
                y: 0,
                width: 69,
                height: 84
            }
        );
        // 46 × 1.25 = 57.5: each edge rounds on its own, so the button and Close share the edge between them.
        let at_125 = BESIDE_CLOSE.in_pixels(1375, 1.25, 0);
        assert_eq!(at_125.x + at_125.width, 1375 - 58);
        assert_eq!(at_125.x, 1375 - 115);
    }

    #[test]
    fn leaves_the_top_resize_border_out() {
        assert_eq!(
            BESIDE_CLOSE.in_pixels(1100, 1.0, 4),
            PixelRect {
                x: 1008,
                y: 4,
                width: 46,
                height: 52
            }
        );
    }

    #[test]
    fn never_gives_a_negative_size() {
        let flat = ButtonPlace {
            height: 2.0,
            ..BESIDE_CLOSE
        };
        assert_eq!(flat.in_pixels(1100, 1.0, 4).height, 0);
    }
}
