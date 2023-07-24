use scene::{Id, Layer};

use crate::{
    bridge::log,
    dom::{element::Element, icon::Icon, input::InputGroup},
    start::VpRef,
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
    vp: VpRef,
}

impl LayersMenu {
    pub fn new(vp: VpRef) -> Self {
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
        let vp_ref = vp.clone();
        button.set_onclick(Box::new(move |_| {
            if let Ok(mut lock) = vp_ref.try_lock() {
                lock.scene.new_layer();
            } else {
                log("Failed to lock viewport to add layer.");
            }
        }));

        Self { root, list, vp }
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

            let mut input = InputGroup::new(self.vp.clone());
            input.root().add_class("mt-1");
            self.list.append_child(input.root());

            let id = layer.id;
            input.add_radio("selected-layer", id == selected, move |vp| {
                vp.scene.select_layer(id)
            });

            input.add_toggle_string("Title", false, move |vp, title| {
                vp.scene.rename_layer(id, title);
            });
            input.set_string("Title", &layer.title);

            let locked = layer.locked;
            input.add_button(if locked { Icon::Lock } else { Icon::Unlock }, move |vp| {
                vp.scene.set_layer_locked(id, !locked)
            });

            let visible = layer.visible;
            input.add_button(
                if visible { Icon::Eye } else { Icon::EyeSlash },
                move |vp| {
                    vp.scene.set_layer_visible(id, !visible);
                },
            );

            input.add_button(Icon::Up, move |vp| vp.scene.move_layer(id, true));
            input.add_button(Icon::Down, move |vp| vp.scene.move_layer(id, false));

            input.add_button(Icon::Trash, move |vp| vp.scene.remove_layer(id));
        }
    }
}
