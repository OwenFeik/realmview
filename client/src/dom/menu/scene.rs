use crate::dom::element::Element;
use crate::dom::input::InputGroup;
use crate::interactor::details::SceneDetails;

pub struct SceneMenu {
    inputs: InputGroup,
}

impl SceneMenu {
    pub fn new() -> Self {
        let mut inputs = InputGroup::new();

        inputs.add_float("Width", Some(0), Some(scene::Scene::MAX_SIZE as i32));
        inputs.add_float("Height", Some(0), Some(scene::Scene::MAX_SIZE as i32));
        inputs.add_line();
        inputs.add_checkbox("Fog of War");
        inputs.add_float("Brush", Some(1), Some(20));
        inputs.add_line();
        inputs.add_select("Change Scene", &[]);

        Self { inputs }
    }

    pub fn root(&self) -> &Element {
        &self.inputs.root
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
        self.set_scene(details.key);
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

    pub fn set_scene(&self, scene: Option<String>) {
        self.inputs
            .set_value_string("Change Scene", &scene.unwrap_or_default());
    }

    pub fn set_scene_list(&mut self, scenes: Vec<(String, String)>) {
        let selected = self.scene();
        self.inputs.set_options("Change Scene", &scenes);
        if let Some(key) = selected {
            self.inputs.set_value_string("Change Scene", &key);
        }
    }
}
