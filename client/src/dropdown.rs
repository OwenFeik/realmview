use std::rc::Rc;

use parking_lot::Mutex;

use crate::bridge::add_dropdown_entry;

#[derive(Clone, Copy)]
pub enum DropdownEvent {
    Clone,
    Delete,
    Group,
    Layer(scene::Id),
    Ungroup,
}

pub struct Dropdown {
    event: Rc<Mutex<Option<DropdownEvent>>>,
}

impl Dropdown {
    pub fn new() -> Self {
        let dropdown = Dropdown {
            event: Rc::new(Mutex::new(None)),
        };

        for (label, event_type) in [
            ("Clone", DropdownEvent::Clone),
            ("Delete", DropdownEvent::Delete),
            ("Group Selection", DropdownEvent::Group),
            ("Ungroup", DropdownEvent::Ungroup),
        ] {
            let event = dropdown.event.clone();
            add_dropdown_entry(
                label,
                Box::new(move || {
                    event.lock().replace(event_type);
                }),
            )
            .ok();
        }

        dropdown
    }

    pub fn event(&mut self) -> Option<DropdownEvent> {
        self.event.lock().take()
    }
}
