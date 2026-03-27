use gpui::*;

pub struct Theme;

impl Theme {
    // Background colors
    pub fn bg_primary() -> Hsla {
        hsla(235.0 / 360.0, 0.33, 0.14, 1.0)
    }

    pub fn bg_secondary() -> Hsla {
        hsla(216.0 / 360.0, 0.44, 0.16, 1.0)
    }

    pub fn bg_tertiary() -> Hsla {
        hsla(210.0 / 360.0, 0.50, 0.12, 1.0)
    }

    pub fn bg_panel() -> Hsla {
        hsla(230.0 / 360.0, 0.30, 0.12, 1.0)
    }

    // Text colors
    pub fn text_primary() -> Hsla {
        hsla(0.0, 0.0, 0.90, 1.0)
    }

    pub fn text_secondary() -> Hsla {
        hsla(0.0, 0.0, 0.60, 1.0)
    }

    pub fn text_muted() -> Hsla {
        hsla(0.0, 0.0, 0.40, 1.0)
    }

    // Status colors
    pub fn green() -> Hsla {
        hsla(140.0 / 360.0, 0.60, 0.45, 1.0)
    }

    pub fn yellow() -> Hsla {
        hsla(45.0 / 360.0, 0.80, 0.50, 1.0)
    }

    pub fn red() -> Hsla {
        hsla(0.0, 0.70, 0.50, 1.0)
    }

    pub fn purple() -> Hsla {
        hsla(260.0 / 360.0, 0.60, 0.55, 1.0)
    }

    // Border
    pub fn border() -> Hsla {
        hsla(0.0, 0.0, 0.20, 1.0)
    }

    // Button styles
    pub fn button_bg() -> Hsla {
        hsla(0.0, 0.0, 0.18, 1.0)
    }

    pub fn button_hover() -> Hsla {
        hsla(0.0, 0.0, 0.25, 1.0)
    }

    pub fn button_primary() -> Hsla {
        hsla(240.0 / 360.0, 0.50, 0.40, 1.0)
    }

    pub fn button_primary_hover() -> Hsla {
        hsla(240.0 / 360.0, 0.50, 0.48, 1.0)
    }

    pub fn button_green() -> Hsla {
        hsla(140.0 / 360.0, 0.50, 0.25, 1.0)
    }

    pub fn button_green_hover() -> Hsla {
        hsla(140.0 / 360.0, 0.50, 0.32, 1.0)
    }

    pub fn button_red() -> Hsla {
        hsla(0.0, 0.50, 0.25, 1.0)
    }

    pub fn button_red_hover() -> Hsla {
        hsla(0.0, 0.50, 0.32, 1.0)
    }

    // Tab
    pub fn tab_active() -> Hsla {
        hsla(216.0 / 360.0, 0.44, 0.20, 1.0)
    }

    pub fn tab_inactive() -> Hsla {
        hsla(0.0, 0.0, 0.14, 1.0)
    }

    // Selection
    pub fn selection_bg() -> Hsla {
        hsla(216.0 / 360.0, 0.50, 0.22, 1.0)
    }

    // Image preview background (white for logos)
    pub fn image_bg() -> Hsla {
        hsla(0.0, 0.0, 0.95, 1.0)
    }
}
