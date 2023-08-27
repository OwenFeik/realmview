use super::element::Element;

#[derive(Clone, Copy, Debug)]
pub enum Icon {
    Arrows,
    Brush,
    Circle,
    Cursor,
    Down,
    Edit,
    Eye,
    EyeSlash,
    Fog,
    Line,
    Lock,
    Ok,
    Plus,
    Square,
    Target,
    Trash,
    Unlock,
    Up,
}

impl Icon {
    pub fn class(&self) -> String {
        let suf = match self {
            Icon::Arrows => "arrows-move",
            Icon::Brush => "brush",
            Icon::Circle => "circle",
            Icon::Cursor => "cursor",
            Icon::Down => "chevron-down",
            Icon::Edit => "pencil-square",
            Icon::Eye => "eye",
            Icon::EyeSlash => "eye-slash",
            Icon::Fog => "cloud-fog2",
            Icon::Line => "slash-lg",
            Icon::Lock => "lock",
            Icon::Ok => "check-circle",
            Icon::Plus => "plus",
            Icon::Square => "square",
            Icon::Target => "bullseye",
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
