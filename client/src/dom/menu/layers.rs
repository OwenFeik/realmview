use std::{
    rc::Rc,
    sync::atomic::{AtomicBool, Ordering},
};

use scene::{Id, Layer};

use crate::{
    dom::{element::Element, icon::Icon, input::InputGroup},
    start::VpRef,
};

pub struct LayerInfo {
    id: Id,
    title: String,
    z: i32,
    visible: bool,
    locked: bool,
    n_sprites: usize,
}

impl LayerInfo {
    pub fn from(layer: &Layer) -> Self {
        LayerInfo {
            id: layer.id,
            title: layer.title.clone(),
            z: layer.z,
            visible: layer.visible,
            locked: layer.locked,
            n_sprites: layer.sprites.len(),
        }
    }
}

pub struct LayersMenu {
    root: Element,
    list: Element,
    new: Rc<AtomicBool>,
    vp: VpRef,
}

impl LayersMenu {
    pub fn new(vp: VpRef) -> Self {
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

        Self {
            root,
            list,
            new,
            vp,
        }
    }

    pub fn root(&self) -> &Element {
        &self.root
    }

    pub fn new_layer(&self) -> bool {
        self.new.swap(false, Ordering::Relaxed)
    }

    pub fn update(&self, layers: &[LayerInfo]) {
        self.list.clear();
        let mut background = false;
        for layer in layers {
            if layer.z < 0 && !background {
                self.list.child("hr").with_class("mb-0").with_class("mt-1");
                background = true;
            }

            let mut input = InputGroup::new(self.vp.clone());
            let id = layer.id;
            input.add_toggle_string(
                "Title",
                false,
                Box::new(move |vp, title| {
                    vp.lock().scene.rename_layer(id, title);
                }),
            );
            input.set_value_string("Title", &layer.title);

            let locked = layer.locked;
            input.add_button(
                if locked { Icon::Lock } else { Icon::Unlock },
                Box::new(move |vp| vp.lock().scene.set_layer_locked(id, !locked)),
            );

            let visible = layer.visible;
            input.add_button(
                if visible { Icon::Eye } else { Icon::EyeSlash },
                Box::new(move |vp| {
                    vp.lock().scene.set_layer_visible(id, !visible);
                }),
            );
            // input.add_toggle("Up", Icon::Up, Icon::Up, Box::new(|_| {}));
            // input.add_toggle("Down", Icon::Down, Icon::Down, Box::new(|_| {}));
            input.root.add_class("mt-1");
            self.list.append_child(&input.root);
        }
    }
}
