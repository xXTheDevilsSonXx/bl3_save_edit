use iced::{button, Color};

pub struct ItemEditorButtonStyle {
    pub is_active: bool,
}

impl button::StyleSheet for ItemEditorButtonStyle {
    fn active(&self) -> button::Style {
        let (background, text_color) = if self.is_active {
            (
                Some(Color::from_rgb8(30, 30, 30).into()),
                Color::from_rgb8(255, 255, 255),
            )
        } else {
            (
                Some(Color::from_rgb8(22, 22, 22).into()),
                Color::from_rgb8(220, 220, 220),
            )
        };

        button::Style {
            background,
            text_color,
            border_width: 0.0,
            ..button::Style::default()
        }
    }

    fn hovered(&self) -> button::Style {
        button::Style {
            background: Some(Color::from_rgb8(30, 30, 30).into()),
            border_width: 0.0,
            text_color: Color::from_rgb8(255, 255, 255),
            ..button::Style::default()
        }
    }

    fn pressed(&self) -> button::Style {
        button::Style {
            background: Some(Color::from_rgb8(25, 25, 25).into()),
            border_width: 0.0,
            text_color: Color::from_rgb8(220, 220, 220),
            ..button::Style::default()
        }
    }
}