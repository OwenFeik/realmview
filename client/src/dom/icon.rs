#[derive(Clone, Copy, Debug)]
pub enum Icon {
    Down,
    Eye,
    EyeSlash,
    Lock,
    Unlock,
    Up,
    Trash,
}

impl Icon {
    pub fn class(&self) -> String {
        let suf = match self {
            Icon::Down => "chevron-down",
            Icon::Eye => "eye",
            Icon::EyeSlash => "eye-slash",
            Icon::Lock => "lock",
            Icon::Unlock => "unlock",
            Icon::Up => "chevron-up",
            Icon::Trash => "trash3",
        };
        format!("bi-{suf}")
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
