use super::element::Element;
use crate::start::VpRef;

mod layers;
mod scene;

pub use layers::LayerInfo;

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
    let heading = format!("{}_heading", &prefix);
    let collapse = format!("{}_collapse", &prefix);
    item.child("h2")
        .with_attr("id", &heading)
        .with_class("accordion-header")
        .child("button")
        .with_classes(&["accordion-button", "shadow-none", "collapsed"])
        .with_attrs(&[
            ("type", "button"),
            ("data-bs-toggle", "collapse"),
            ("data-bs-target", &format!("#{collapse}")),
            ("aria-expanded", "false"),
            ("aria-controls", &format!("#{collapse}")),
        ])
        .with_text(key);
    item.child("div")
        .with_attr("id", &collapse)
        .with_classes(&["accordion-collapse", "collapse"])
        .with_attr("aria-labelledby", &format!("#{heading}"))
        .with_child(inputs);
}

pub struct Menu {
    pub layers: self::layers::LayersMenu,
    pub scene: self::scene::SceneMenu,
}

impl Menu {
    pub fn new(vp: VpRef) -> Self {
        let menu = Self {
            layers: layers::LayersMenu::new(vp.clone()),
            scene: scene::SceneMenu::new(vp),
        };

        add_to_menu("Layers", menu.layers.root());
        add_to_menu("Scene", menu.scene.root());
        menu
    }
}
