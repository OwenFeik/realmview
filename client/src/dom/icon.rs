use super::element::Element;

#[derive(Clone, Copy, Debug)]
pub enum Icon {
    Brush,
    Circle,
    Down,
    Edit,
    Eye,
    EyeSlash,
    Line,
    Lock,
    Ok,
    Plus,
    Square,
    Trash,
    Unlock,
    Up,
}

impl Icon {
    pub fn class(&self) -> String {
        let suf = match self {
            Icon::Brush => "brush",
            Icon::Circle => "circle",
            Icon::Down => "chevron-down",
            Icon::Edit => "pencil-square",
            Icon::Eye => "eye",
            Icon::EyeSlash => "eye-slash",
            Icon::Line => "slash-lg",
            Icon::Lock => "lock",
            Icon::Ok => "check-circle",
            Icon::Plus => "plus",
            Icon::Square => "square",
            Icon::Trash => "trash3",
            Icon::Unlock => "unlock",
            Icon::Up => "chevron-up",
        };
        format!("bi-{suf}")
    }

    pub fn element(&self) -> Element {
        Element::new("i").with_class(&self.class())
    }

    pub fn opposite(&self) -> Self {
        match self {
            Icon::Down => Icon::Up,
            Icon::Up => Icon::Down,
            Icon::Eye => Icon::EyeSlash,
            Icon::EyeSlash => Icon::Eye,
            Icon::Lock => Icon::Unlock,
            Icon::Unlock => Icon::Lock,
            _ => *self,
        }
    }
}
