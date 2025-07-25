use crate::dom::element::Element;
use crate::dom::icon::Icon;
use crate::dom::input::InputGroup;
use crate::interactor::details::SceneDetails;

pub struct SceneMenu {
    inputs: InputGroup,
}

impl SceneMenu {
    pub fn new() -> Self {
        let mut inputs = InputGroup::new();

        inputs.add_toggle_string("Title", true, |vp, title| {
            vp.int.scene_details(SceneDetails {
                title: Some(title),
                ..Default::default()
            })
        });
        inputs.add_line();
        inputs.add_float_handler(
            "Width",
            Some(0),
            Some(scene::Scene::MAX_SIZE as i32),
            None,
            |vp, w| {
                vp.int.scene_details(SceneDetails {
                    w: Some(w as u32),
                    ..Default::default()
                });
            },
        );
        inputs.add_float_handler(
            "Height",
            Some(0),
            Some(scene::Scene::MAX_SIZE as i32),
            None,
            |vp, h| {
                vp.int.scene_details(SceneDetails {
                    h: Some(h as u32),
                    ..Default::default()
                });
            },
        );
        inputs.add_line();
        inputs.add_checkbox_handler("Fog of War", |vp, active| {
            vp.int.scene_details(SceneDetails {
                fog: Some(active),
                ..Default::default()
            });
        });
        inputs.add_float_handler("Brush", Some(1), Some(20), Some(0.5), |vp, brush| {
            vp.int.set_fog_brush(brush)
        });
        inputs.add_line();
        inputs.add_select_handler("Change Scene", &[], |vp, uuid| {
            if let Ok(uuid) = uuid::Uuid::try_parse(&uuid) {
                crate::bridge::upload_thumbnail(&vp.int.scene_uuid());
                vp.int.change_scene(uuid);
            }
        });
        inputs.add_button(Icon::PlusSquare, |vp| vp.int.new_scene());

        Self { inputs }
    }

    pub fn root(&self) -> &Element {
        self.inputs.root()
    }

    pub fn title(&self) -> Option<String> {
        self.inputs.get_string("Title")
    }

    pub fn width(&self) -> Option<u32> {
        self.inputs.get_u32("Width")
    }

    pub fn height(&self) -> Option<u32> {
        self.inputs.get_u32("Height")
    }

    pub fn fog_of_war(&self) -> Option<bool> {
        self.inputs.get_bool("Fog of War")
    }

    pub fn set_details(&mut self, details: SceneDetails) {
        if let Some(title) = details.title {
            self.inputs.set_string("Title", &title);
        }
        self.inputs.set_float(
            "Width",
            details.w.unwrap_or(scene::Scene::DEFAULT_SIZE) as f32,
        );
        self.inputs.set_float(
            "Height",
            details.h.unwrap_or(scene::Scene::DEFAULT_SIZE) as f32,
        );
        self.inputs
            .set_bool("Fog of War", details.fog.unwrap_or(false));
        if let Some(scene) = details.uuid {
            self.set_scene(scene.simple().to_string());
        }
    }

    pub fn set_brush(&mut self, brush: u32) {
        self.inputs.set_float("Brush", brush as f32);
    }

    pub fn details(&self) -> SceneDetails {
        SceneDetails {
            title: self.title(),
            w: self.width(),
            h: self.height(),
            fog: self.fog_of_war(),
            ..Default::default()
        }
    }

    pub fn fog_brush(&self) -> f32 {
        self.inputs
            .get_f32("Brush")
            .unwrap_or(crate::interactor::Interactor::DEFAULT_FOG_BRUSH)
    }

    pub fn set_fog_brush(&self, brush: f32) {
        self.inputs.set_float("Brush", brush);
    }

    pub fn scene(&self) -> Option<String> {
        self.inputs.get_string("Change Scene")
    }

    pub fn set_scene(&self, scene: String) {
        self.inputs.set_string("Change Scene", &scene);
    }

    pub fn set_scene_list(&mut self, scenes: Vec<(String, String)>) {
        let selected = self.scene();
        self.inputs.set_options("Change Scene", &scenes);
        if let Some(key) = selected {
            self.inputs.set_string("Change Scene", &key);
        }
    }
}
