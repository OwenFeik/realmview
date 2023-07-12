use crate::{
    dom::{element::Element, icon::Icon, input::InputGroup},
    interactor::details::SpriteDetails,
    start::VpRef,
    viewport::DrawTool,
};

pub struct DrawMenu {
    inputs: InputGroup,
    tool: DrawTool,
}

impl DrawMenu {
    const COLOUR: &str = "Colour";
    const DRAW_TOOL: &str = "draw_tool";
    const CAP_START: &str = "Start";
    const CAP_END: &str = "End";
    const STROKE: &str = "Stroke";

    pub fn new(vp: VpRef) -> Self {
        let mut inputs = InputGroup::new(vp);

        inputs.add_colour(Self::COLOUR);
        inputs.set_value_colour(Self::COLOUR, random_bright_colour());

        inputs.add_line();

        inputs.add_float(Self::STROKE, Some(0), None, Some(0.1));
        inputs.set_value_float(Self::STROKE, scene::Sprite::DEFAULT_STROKE);

        inputs.add_checkbox("Solid");
        inputs.set_value_bool("Solid", false);

        inputs.add_line();

        const CAP_OPTIONS: &[(&str, &str)] =
            &[("Arrow", "arrow"), ("Round", "round"), ("None", "none")];
        inputs.add_select(Self::CAP_START, CAP_OPTIONS);
        inputs.set_value_string(Self::CAP_START, cap_to_str(scene::Cap::DEFAULT_START));
        inputs.add_select(Self::CAP_END, CAP_OPTIONS);
        inputs.set_value_string(Self::CAP_END, cap_to_str(scene::Cap::DEFAULT_END));

        inputs.add_line();

        inputs.add_icon_radio_handler(
            Self::DRAW_TOOL,
            &[Icon::Brush, Icon::Line, Icon::Square, Icon::Circle],
            |vp, icon| {
                vp.set_draw_tool(match icon {
                    Icon::Brush => DrawTool::Freehand,
                    Icon::Line => DrawTool::Line,
                    Icon::Square => DrawTool::Rectangle,
                    Icon::Circle => DrawTool::Ellipse,
                    _ => DrawTool::Freehand,
                });
            },
        );
        inputs.set_selected_icon_radio(Self::DRAW_TOOL, Icon::Brush);

        Self {
            inputs,
            tool: DrawTool::Freehand,
        }
    }

    pub fn root(&self) -> &Element {
        &self.inputs.root
    }

    pub fn change_stroke(&self, delta: f32) {
        /// This coefficient is based on scroll delta sizes observed in firefox.
        /// Could maybe be abstracted a bit more.
        const COEFF: f32 = -1.0 / (114.0 * 4.0);

        let old = self
            .inputs
            .value_float(Self::STROKE)
            .map(|s| s as f32)
            .unwrap_or(scene::Sprite::DEFAULT_STROKE);

        let new = (old + delta * COEFF).max(0.0);
        self.inputs.set_value_float(Self::STROKE, new);
    }

    pub fn details(&self) -> SpriteDetails {
        SpriteDetails {
            shape: match self.tool {
                DrawTool::Ellipse => Some(scene::Shape::Ellipse),
                DrawTool::Rectangle => Some(scene::Shape::Rectangle),
                DrawTool::Freehand | DrawTool::Line => None,
            },
            stroke: self.inputs.value_f32(Self::STROKE),
            colour: self.inputs.value_colour(Self::COLOUR),
            cap_start: self
                .inputs
                .value_string(Self::CAP_START)
                .map(|s| str_to_cap(&s)),
            cap_end: self
                .inputs
                .value_string(Self::CAP_END)
                .map(|s| str_to_cap(&s)),
            drawing_mode: match self.tool {
                DrawTool::Freehand => Some(scene::DrawingMode::Freehand),
                DrawTool::Line => Some(scene::DrawingMode::Line),
                DrawTool::Ellipse | DrawTool::Rectangle => None,
            },
            ..Default::default()
        }
    }

    fn update(&self, details: &SpriteDetails) {
        if let Some(value) = details.stroke {
            self.inputs.set_value_float(Self::STROKE, value);
        }

        if let Some(value) = details.colour {
            self.inputs.set_value_colour(Self::COLOUR, value);
        }

        if let Some(cap) = details.cap_start {
            self.inputs
                .set_value_string(Self::CAP_START, cap_to_str(cap));
        }

        if let Some(cap) = details.cap_end {
            self.inputs.set_value_string(Self::CAP_END, cap_to_str(cap));
        }
    }

    pub fn set_draw_tool(&mut self, draw_tool: DrawTool) {
        let mut deets: crate::interactor::details::SpriteDetails = Default::default();
        let icon = match draw_tool {
            DrawTool::Ellipse => {
                deets.shape = Some(::scene::Shape::Ellipse);
                Icon::Circle
            }
            DrawTool::Freehand => {
                deets.shape = None;
                deets.drawing_mode = Some(::scene::DrawingMode::Freehand);
                deets.cap_end = Some(::scene::Cap::Round);
                Icon::Brush
            }
            DrawTool::Line => {
                deets.shape = None;
                deets.drawing_mode = Some(::scene::DrawingMode::Line);
                deets.cap_end = Some(::scene::Cap::Arrow);
                Icon::Line
            }
            DrawTool::Rectangle => {
                deets.shape = Some(::scene::Shape::Rectangle);
                Icon::Square
            }
        };

        self.update(&deets);
        self.inputs.set_selected_icon_radio(Self::DRAW_TOOL, icon);
        self.tool = draw_tool;
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

fn is_bright_colour(colour: scene::Colour) -> bool {
    const THRESHOLD: f32 = 2.0;
    colour.r() + colour.g() + colour.b() >= THRESHOLD
}

fn random_bright_colour() -> scene::Colour {
    use crate::bridge::rand;

    loop {
        let colour = scene::Colour([rand(), rand(), rand(), scene::Colour::DEFAULT.a()]);
        if is_bright_colour(colour) {
            break colour;
        }
    }
}
