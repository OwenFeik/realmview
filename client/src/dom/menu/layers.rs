use scene::{Id, Layer};

use crate::{
    dom::{element::Element, icon::Icon, input::InputGroup},
    viewport,
};

pub struct LayerInfo {
    pub id: Id,
    pub title: String,
    pub z: i32,
    pub visible: bool,
    pub locked: bool,
    pub n_sprites: usize,
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
}

impl LayersMenu {
    pub fn new() -> Self {
        let root = Element::default();

        let list = root
            .child("ul")
            .with_classes(&["list-unstyled", "mb-0", "pt-1"]);

        let mut button = root
            .child("button")
            .with_classes(&["btn", "btn-primary", "btn-sm", "mt-1"])
            .with_attr("type", "button");
        button.child("span").set_text("Add");
        button.icon(Icon::Plus);
        button.set_onclick(Box::new(move |_| {
            viewport::lock_and(|vp| vp.int.new_layer())
        }));

        Self { root, list }
    }

    pub fn root(&self) -> &Element {
        &self.root
    }

    pub fn update(&self, selected: Id, layers: &[LayerInfo]) {
        self.list.clear();
        let mut background = false;
        for layer in layers {
            if layer.z < 0 && !background {
                self.list.child("hr").with_class("mb-0").with_class("mt-1");
                background = true;
            }

            let mut input = InputGroup::new();
            input.root().add_class("mt-1");
            self.list.append_child(input.root());

            let id = layer.id;
            input.add_radio("selected-layer", id == selected, move |vp| {
                vp.int.select_layer(id)
            });

            input.add_toggle_string("Title", false, move |vp, title| {
                vp.int.rename_layer(id, title);
            });
            input.set_string("Title", &layer.title);

            let locked = layer.locked;
            input.add_button(if locked { Icon::Lock } else { Icon::Unlock }, move |vp| {
                vp.int.set_layer_locked(id, !locked)
            });

            let visible = layer.visible;
            input.add_button(
                if visible { Icon::Eye } else { Icon::EyeSlash },
                move |vp| {
                    vp.int.set_layer_visible(id, !visible);
                },
            );

            input.add_button(Icon::Up, move |vp| vp.int.move_layer(id, true));
            input.add_button(Icon::Down, move |vp| vp.int.move_layer(id, false));

            input.add_button(Icon::Trash, move |vp| vp.int.remove_layer(id));
        }
    }
}
