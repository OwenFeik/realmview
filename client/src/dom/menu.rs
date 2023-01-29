use crate::{
    dom::{element::Element, input::InputGroup},
    interactor::details::SceneDetails,
};

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
    pub fn new(details: SceneDetails, brush: u32) -> Self {
        let mut inputs = InputGroup::new();

        inputs.add_float("Width", Some(0), Some(scene::Scene::MAX_SIZE as i32));
        inputs.add_float("Height", Some(0), Some(scene::Scene::MAX_SIZE as i32));
        inputs.add_line();
        inputs.add_checkbox("Fog of War");
        inputs.add_float("Brush", Some(1), Some(20));
        inputs.add_line();
        inputs.add_select("Change Scene", &[("Test", "SceneKey")]);

        add_to_menu("Scene", &inputs.root);

        let mut menu = Self { inputs };
        menu.set_details(details, brush);
        menu
    }

    pub fn changed(&mut self) -> bool {
        self.inputs.handle_change()
    }

    pub fn width(&self) -> Option<u32> {
        self.inputs.value_unsigned("Width")
    }

    pub fn height(&self) -> Option<u32> {
        self.inputs.value_unsigned("Height")
    }

    pub fn fog_of_war(&self) -> Option<bool> {
        self.inputs.value_bool("Fog of War")
    }

    pub fn set_details(&mut self, details: SceneDetails, brush: u32) {
        self.inputs.set_value_float(
            "Width",
            details.w.unwrap_or(scene::Scene::DEFAULT_SIZE) as f64,
        );
        self.inputs.set_value_float(
            "Height",
            details.h.unwrap_or(scene::Scene::DEFAULT_SIZE) as f64,
        );
        self.inputs
            .set_value_bool("Fog of War", details.fog.unwrap_or(false));
        self.inputs.set_value_float("Brush", brush as f64);
    }

    pub fn details(&self) -> SceneDetails {
        SceneDetails {
            w: self.width(),
            h: self.height(),
            fog: self.fog_of_war(),
            ..Default::default()
        }
    }

    pub fn fog_brush(&self) -> u32 {
        self.inputs
            .value_unsigned("Brush")
            .unwrap_or(crate::interactor::Interactor::DEFAULT_FOG_BRUSH)
    }

    pub fn set_fog_brush(&self, brush: u32) {
        self.inputs.set_value_float("Brush", brush as f64);
    }

    pub fn scene(&self) -> Option<String> {
        self.inputs.value_string("Change Scene")
    }
}
