//! Looking closely at a picture: fitted to its panel at first, then zoomed in on the place under
//! the pointer and moved around by dragging or scrolling. Sizes are in pixels; the pan is how far
//! the picture's middle sits from the panel's.

/// The most a picture is enlarged: enough to count a screenshot's pixels.
pub const MOST: f32 = 16.;

/// How a picture is shown in a panel.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Zoom {
    /// How much it's enlarged; `None` while it's fitted to the panel.
    pub scale: Option<f32>,
    pub pan: (f32, f32),
}

/// Where the picture lands in the panel: its left, top, width and height.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placed {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

/// The scale that fits `picture` whole into `panel`, never enlarging it past its own size.
pub fn fit(picture: (f32, f32), panel: (f32, f32)) -> f32 {
    if picture.0 <= 0. || picture.1 <= 0. {
        return 1.;
    }
    (panel.0 / picture.0).min(panel.1 / picture.1).clamp(f32::MIN_POSITIVE, 1.)
}

impl Zoom {
    /// The scale the picture is shown at now.
    pub fn current(self, picture: (f32, f32), panel: (f32, f32)) -> f32 {
        self.scale.unwrap_or_else(|| fit(picture, panel))
    }

    /// Enlarges (`by` above 1) or shrinks the picture, keeping the point under `at` (in the panel,
    /// from its top left) where it is. Shrunk to the fitted size or below, it's fitted again.
    #[must_use]
    pub fn by(self, by: f32, at: (f32, f32), picture: (f32, f32), panel: (f32, f32)) -> Self {
        let fitted = fit(picture, panel);
        let before = self.current(picture, panel);
        let after = (before * by).min(MOST);
        if after <= fitted * 1.001 {
            return Self::default();
        }
        // The picture's point under `at` stays under it.
        let from_middle = (at.0 - panel.0 / 2., at.1 - panel.1 / 2.);
        let ratio = after / before;
        let pan = (
            from_middle.0 - (from_middle.0 - self.pan.0) * ratio,
            from_middle.1 - (from_middle.1 - self.pan.1) * ratio,
        );
        Self { scale: Some(after), pan }.held(picture, panel)
    }

    /// Moves the picture by `by`, as far as it has more to show that way.
    #[must_use]
    pub fn moved(self, by: (f32, f32), picture: (f32, f32), panel: (f32, f32)) -> Self {
        if self.scale.is_none() {
            return self;
        }
        Self { pan: (self.pan.0 + by.0, self.pan.1 + by.1), ..self }.held(picture, panel)
    }

    /// Fitted, or at its actual size (twice that for a picture smaller than the panel) with the
    /// point under `at` kept where it is: what a double click does.
    #[must_use]
    pub fn toggled(self, at: (f32, f32), picture: (f32, f32), panel: (f32, f32)) -> Self {
        if self.scale.is_some() {
            return Self::default();
        }
        let fitted = fit(picture, panel);
        let wanted = if fitted >= 1. { 2. } else { 1. };
        self.by(wanted / fitted, at, picture, panel)
    }

    /// Keeps the picture's edges from leaving the panel's while it's larger than the panel, and
    /// centred along a side where it's smaller.
    fn held(self, picture: (f32, f32), panel: (f32, f32)) -> Self {
        let scale = self.current(picture, panel);
        let spare = |shown: f32, room: f32| ((shown - room) / 2.).max(0.);
        let (x, y) = (spare(picture.0 * scale, panel.0), spare(picture.1 * scale, panel.1));
        Self { pan: (self.pan.0.clamp(-x, x), self.pan.1.clamp(-y, y)), ..self }
    }

    /// Where the picture is drawn.
    pub fn place(self, picture: (f32, f32), panel: (f32, f32)) -> Placed {
        let scale = self.current(picture, panel);
        let (width, height) = (picture.0 * scale, picture.1 * scale);
        Placed { left: (panel.0 - width) / 2. + self.pan.0, top: (panel.1 - height) / 2. + self.pan.1, width, height }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHOT: (f32, f32) = (2000., 1000.);
    const PANEL: (f32, f32) = (1000., 1000.);

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    #[test]
    fn a_picture_fits_its_panel_but_is_never_enlarged_to_fit() {
        assert!(near(fit(SHOT, PANEL), 0.5));
        assert!(near(fit((200., 100.), PANEL), 1.));
    }

    #[test]
    fn zooming_keeps_the_point_under_the_pointer_still() {
        let at = (900., 500.);
        let zoom = Zoom::default().by(2., at, SHOT, PANEL);
        assert_eq!(zoom.scale, Some(1.));
        // The picture's point under the pointer before (x 1800 of 2000) is still under it.
        let placed = zoom.place(SHOT, PANEL);
        assert!(near(placed.left + 1800., 900.));
    }

    #[test]
    fn shrinking_back_to_the_fitted_size_fits_it_again() {
        let zoom = Zoom::default().by(2., (500., 500.), SHOT, PANEL).by(0.4, (500., 500.), SHOT, PANEL);
        assert_eq!(zoom, Zoom::default());
    }

    #[test]
    fn a_zoomed_picture_moves_only_as_far_as_its_edges() {
        let zoom = Zoom { scale: Some(1.), pan: (0., 0.) }.moved((5000., 300.), SHOT, PANEL);
        // 2000 wide in a 1000 panel: 500 to spare either way; 1000 tall in 1000: none.
        assert_eq!(zoom.pan, (500., 0.));
        assert_eq!(Zoom::default().moved((50., 50.), SHOT, PANEL), Zoom::default());
    }

    #[test]
    fn a_double_click_shows_the_actual_size_then_fits_again() {
        let actual = Zoom::default().toggled((500., 500.), SHOT, PANEL);
        assert_eq!(actual.scale, Some(1.));
        assert_eq!(actual.toggled((500., 500.), SHOT, PANEL), Zoom::default());
        let small = Zoom::default().toggled((500., 500.), (200., 100.), PANEL);
        assert_eq!(small.scale, Some(2.));
    }

    #[test]
    fn zoom_stops_at_the_most() {
        let zoom = Zoom::default().by(1000., (500., 500.), SHOT, PANEL);
        assert_eq!(zoom.scale, Some(MOST));
    }
}
