use crate::{
    dom::{element::Element, icon::Icon, input::InputGroup},
    interactor::details::SpriteDetails,
    start::VpRef,
};

pub struct DrawMenu {
    inputs: InputGroup,
}

impl DrawMenu {
    const COLOUR: &str = "Colour";
    const OPACITY: &str = "Opacity";
    const CAP_START: &str = "Start";
    const CAP_END: &str = "End";
    const STROKE: &str = "Stroke";

    pub fn new(vp: VpRef) -> Self {
        let mut inputs = InputGroup::new(vp.clone());

        let details = SpriteDetails {
            stroke: Some(scene::Sprite::DEFAULT_STROKE),
            ..Default::default()
        };

        inputs.add_colour(Self::COLOUR);
        inputs.set_value_colour(Self::COLOUR, details.colour());

        inputs.add_float(Self::OPACITY, Some(0), Some(100));
        inputs.set_value_float(Self::OPACITY, details.colour().a());

        inputs.add_line();

        inputs.add_float(Self::STROKE, Some(0), None);
        inputs.set_value_float(Self::STROKE, details.stroke());

        inputs.add_checkbox("Solid");
        inputs.set_value_bool("Solid", details.stroke() == 0.0);

        inputs.add_line();

        const CAP_OPTIONS: &[(&str, &str)] =
            &[("Arrow", "arrow"), ("Round", "round"), ("None", "none")];
        inputs.add_select(Self::CAP_START, CAP_OPTIONS);
        inputs.set_value_string(
            Self::CAP_START,
            cap_to_str(details.cap_start.unwrap_or(scene::Cap::DEFAULT_START)),
        );
        inputs.add_select(Self::CAP_END, CAP_OPTIONS);
        inputs.set_value_string(
            Self::CAP_END,
            cap_to_str(details.cap_end.unwrap_or(scene::Cap::DEFAULT_END)),
        );

        inputs.add_line();

        inputs.add_icon_radio_handler(
            "draw_tool",
            &[Icon::Brush, Icon::Line, Icon::Square, Icon::Circle],
            Box::new(|vp, idx| {
                let shape = match idx {
                    2 => Some(scene::Shape::Rectangle),
                    3 => Some(scene::Shape::Ellipse),
                    _ => None,
                };
                let drawing_mode = match idx {
                    0 => Some(scene::DrawingMode::Freehand),
                    1 => Some(scene::DrawingMode::Line),
                    _ => None,
                };

                vp.scene.update_draw_details(SpriteDetails {
                    shape,
                    drawing_mode,
                    ..Default::default()
                });
                vp.set_tool(crate::viewport::Tool::Draw);
            }),
        );

        Self { inputs }
    }

    pub fn root(&self) -> &Element {
        &self.inputs.root
    }

    pub fn change_stroke(&mut self, delta: f32) {
        const COEFF: f32 = -1.0 / (114.0 * 4.0);

        let old = self
            .inputs
            .value_float(Self::OPACITY)
            .map(|s| s as f32)
            .unwrap_or(scene::Sprite::DEFAULT_STROKE);

        let new = (old + delta * COEFF).max(0.0);
        self.inputs.set_value_float(Self::OPACITY, new);
    }

    pub fn details(&self) -> SpriteDetails {
        SpriteDetails {
            stroke: self.inputs.value_f32(Self::STROKE),
            colour: self
                .inputs
                .value_colour(Self::COLOUR)
                .map(|c| c.with_opacity(self.inputs.value_f32(Self::OPACITY).unwrap_or(1.0))),
            cap_start: self
                .inputs
                .value_string(Self::CAP_START)
                .map(|s| str_to_cap(&s)),
            cap_end: self
                .inputs
                .value_string(Self::CAP_END)
                .map(|s| str_to_cap(&s)),
            ..Default::default()
        }
    }
}

fn str_to_cap(string: &str) -> scene::Cap {
    match string {
        "arrow" => scene::Cap::Arrow,
        "round" => scene::Cap::Round,
        "none" => scene::Cap::None,
        _ => scene::Cap::None,
    }
}

fn cap_to_str(cap: scene::Cap) -> &'static str {
    match cap {
        scene::Cap::Arrow => "arrow",
        scene::Cap::None => "none",
        scene::Cap::Round => "round",
    }
}
