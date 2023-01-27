use crate::dom::{element::Element, input::InputGroup};

fn add_to_menu(key: &str, inputs: &Element) {
    let el = if let Some(el) = Element::by_id("canvas_menu") {
        el
    } else {
        return;
    };

    let item = el.child("div").with_class("accordion-item");
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

pub struct SceneMenu {
    inputs: InputGroup,
}

impl SceneMenu {
    pub fn new() -> Self {
        let mut inputs = InputGroup::new();

        inputs.add_float("Width", Some(0), Some(scene::Scene::MAX_SIZE as i32));
        inputs.add_float("Height", Some(0), Some(scene::Scene::MAX_SIZE as i32));
        inputs.add_line();
        inputs.add_bool("Fog of War");
        inputs.add_float("Brush", Some(1), Some(20));
        inputs.add_line();
        inputs.add_select("Change Scene", &[("Test", "SceneKey")]);

        add_to_menu("Scene", &inputs.root);

        Self { inputs }
    }
}
