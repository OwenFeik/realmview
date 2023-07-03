use crate::{
    dom::{element::Element, input::InputGroup},
    interactor::details::SpriteDetails,
    start::VpRef,
};

pub struct DrawingMenu {
    root: Element,
    inputs: InputGroup,
}

impl DrawingMenu {
    pub fn new(vp: VpRef) -> Self {
        let root = Element::default();

        let mut inputs = InputGroup::new(vp.clone());

        inputs.add_colour(
            "Colour",
            Box::new(|vp, mut colour| {
                if let Some(opacity) = vp.scene.get_draw_details().colour.map(|c| c.a()) {
                    colour = colour.with_opacity(opacity);
                }

                vp.scene.update_draw_details(SpriteDetails {
                    colour: Some(colour),
                    ..Default::default()
                });
            }),
        );

        inputs.add_float(
            "Opacity",
            Some(0),
            Some(100),
            Box::new(|vp, opacity| {
                let colour = vp
                    .scene
                    .get_draw_details()
                    .colour
                    .unwrap_or(scene::Colour::DEFAULT)
                    .with_opacity(opacity / 100.0);
                vp.scene.update_draw_details(SpriteDetails {
                    colour: Some(colour),
                    ..Default::default()
                });
            }),
        );

        inputs.add_line();

        inputs.add_float(
            "Stroke",
            Some(0),
            None,
            Box::new(|vp, stroke| {
                vp.scene.update_draw_details(SpriteDetails {
                    stroke: Some(stroke),
                    ..Default::default()
                });
            }),
        );

        inputs.add_checkbox(
            "Solid",
            Box::new(|vp, solid| {
                let stroke = if solid {
                    0.0
                } else {
                    vp.scene
                        .get_draw_details()
                        .stroke
                        .unwrap_or(scene::Sprite::DEFAULT_STROKE)
                };

                vp.scene.update_draw_details(SpriteDetails {
                    stroke: Some(stroke),
                    ..Default::default()
                });
            }),
        );

        root.append_child(&inputs.root);

        DrawingMenu { root, inputs }
    }

    pub fn root(&self) -> &Element {
        &self.root
    }
}
