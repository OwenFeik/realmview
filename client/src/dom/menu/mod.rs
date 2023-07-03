use ::scene::Id;

pub use self::dropdown::CanvasDropdownEvent;
pub use self::layers::LayerInfo;
use super::element::Element;
use crate::{interactor::details::SceneDetails, start::VpRef, viewport::ViewportPoint};

mod drawing;
mod dropdown;
mod layers;
mod scene;

fn id(key: &str) -> String {
    format!("#{key}")
}

fn add_to_menu(key: &str, inputs: &Element) {
    let el = if let Some(el) = Element::by_id("canvas_menu") {
        el
    } else {
        return;
    };

    inputs.add_class("p-2");

    let item = el
        .child("div")
        .with_attr("id", &format!("{}_menu", key.to_lowercase()))
        .with_class("accordion-item");
    let prefix = format!("menu_{}", key.to_lowercase());
    let heading = &format!("{}_heading", &prefix);
    let collapse = &format!("{}_collapse", &prefix);
    item.child("h2")
        .with_attr("id", &heading)
        .with_class("accordion-header")
        .child("button")
        .with_classes(&["accordion-button", "shadow-none", "collapsed"])
        .with_attrs(&[
            ("type", "button"),
            ("data-bs-toggle", "collapse"),
            ("data-bs-target", &id(collapse)),
            ("aria-expanded", "false"),
            ("aria-controls", &id(collapse)),
        ])
        .with_text(key);
    item.child("div")
        .with_attr("id", collapse)
        .with_classes(&["accordion-collapse", "collapse"])
        .with_attr("aria-labelledby", &id(heading))
        .with_child(inputs);
}

pub struct Menu {
    dropdown: dropdown::Dropdown,
    layers: layers::LayersMenu,
    scene: scene::SceneMenu,
}

impl Menu {
    pub fn new(vp: VpRef) -> Self {
        let menu = Self {
            dropdown: dropdown::Dropdown::new(),
            layers: layers::LayersMenu::new(vp.clone()),
            scene: scene::SceneMenu::new(vp),
        };

        add_to_menu("Layers", menu.layers.root());
        add_to_menu("Scene", menu.scene.root());
        menu
    }

    pub fn show_dropdown(&self, at: ViewportPoint) {
        self.dropdown.show(at);
    }

    pub fn hide_dropdown(&self) {
        self.dropdown.hide();
    }

    pub fn dropdown_event(&mut self) -> Option<dropdown::CanvasDropdownEvent> {
        self.dropdown.event()
    }

    pub fn set_scene_details(&mut self, details: SceneDetails) {
        self.scene.set_details(details);
    }

    pub fn set_scene(&mut self, key: Option<String>) {
        self.scene.set_scene(key);
    }

    pub fn set_scene_list(&mut self, list: Vec<(String, String)>) {
        self.scene.set_scene_list(list);
    }

    pub fn set_fog_brush(&mut self, brush: u32) {
        self.scene.set_fog_brush(brush);
    }

    pub fn set_layer_info(&mut self, selected: Id, layers: &[LayerInfo]) {
        self.layers.update(selected, layers);
        self.dropdown.update_layers(layers);
    }
}
