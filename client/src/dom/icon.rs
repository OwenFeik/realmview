use super::element::Element;

#[derive(Clone, Copy, Debug)]
pub enum Icon {
    Down,
    Edit,
    Eye,
    EyeSlash,
    Lock,
    Ok,
    Plus,
    Unlock,
    Up,
    Trash,
}

impl Icon {
    pub fn class(&self) -> String {
        let suf = match self {
            Icon::Down => "chevron-down",
            Icon::Edit => "pencil-square",
            Icon::Eye => "eye",
            Icon::EyeSlash => "eye-slash",
            Icon::Lock => "lock",
            Icon::Ok => "check-circle",
            Icon::Plus => "plus",
            Icon::Unlock => "unlock",
            Icon::Up => "chevron-up",
            Icon::Trash => "trash3",
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
