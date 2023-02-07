use std::{
    rc::Rc,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::dom::{element::Element, icon::Icon, input::InputGroup};

pub struct LayersMenu {
    root: Element,
    list: Element,
    new: Rc<AtomicBool>,
}

impl LayersMenu {
    pub fn new() -> Self {
        let root = Element::default();

        let new = Rc::new(AtomicBool::new(false));
        let mut button = root
            .child("button")
            .with_classes(&["btn", "btn-primary", "btn-sm"])
            .with_attr("type", "button");
        button.child("span").set_text("Add");
        button.icon(Icon::Plus);
        let new_ref = new.clone();
        button.set_onclick(Box::new(move |_| {
            new_ref.store(true, Ordering::Relaxed);
        }));

        let list = root
            .child("ul")
            .with_classes(&["list-unstyled", "mb-0", "pt-1"]);

        Self { root, list, new }
    }

    pub fn root(&self) -> &Element {
        &self.root
    }

    pub fn new_layer(&self) -> bool {
        self.new.swap(false, Ordering::Relaxed)
    }

    pub fn update(&self, layers: &[scene::Layer]) {
        self.list.clear();
        for layer in layers {
            let mut input = InputGroup::new();
            input.add_toggle_string("Title", false);
            input.set_value_string("Title", &layer.title);
            input.add_toggle("Locked", Icon::Unlock);
            input.add_toggle("Visible", Icon::Eye);
            input.add_toggle("Up", Icon::Up);
            input.add_toggle("Down", Icon::Down);
            input.root.add_class("mt-1");
            self.list.append_child(&input.root);
        }
    }
}
