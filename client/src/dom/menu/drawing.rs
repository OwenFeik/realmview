use crate::{
    dom::{element::Element, icon::Icon, input::InputGroup},
    interactor::details::SpriteDetails,
    start::VpRef,
};

pub fn menu(vp: VpRef) -> Element {
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

    inputs.add_line();

    const CAP_OPTIONS: &[(&str, &str)] =
        &[("Arrow", "arrow"), ("Round", "round"), ("None", "none")];
    inputs.add_select(
        "Start",
        CAP_OPTIONS,
        Box::new(|vp, cap| {
            vp.scene.update_draw_details(SpriteDetails {
                cap_start: Some(str_to_cap(&cap)),
                ..Default::default()
            });
        }),
    );
    inputs.add_select(
        "End",
        CAP_OPTIONS,
        Box::new(|vp, cap| {
            vp.scene.update_draw_details(SpriteDetails {
                cap_end: Some(str_to_cap(&cap)),
                ..Default::default()
            });
        }),
    );

    inputs.add_line();

    inputs.add_icon_radio(
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

    root.append_child(&inputs.root);
    root
}

fn str_to_cap(string: &str) -> scene::Cap {
    match string {
        "arrow" => scene::Cap::Arrow,
        "round" => scene::Cap::Round,
        "none" => scene::Cap::None,
        _ => scene::Cap::None,
    }
}
