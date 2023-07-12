use std::{
    rc::Rc,
    sync::atomic::{AtomicI64, Ordering},
};

use crate::{
    dom::{element::Element, input::InputGroup},
    interactor::details::SpriteDetails,
    start::VpRef,
};

pub struct SpriteMenu {
    inputs: InputGroup,
    selected_id: Rc<AtomicI64>,
}

impl SpriteMenu {
    const NO_SELECTION: scene::Id = -2;

    const X: &str = "X";
    const Y: &str = "Y";
    const WIDTH: &str = "W";
    const HEIGHT: &str = "H";
    const COLOUR: &str = "Colour";
    const STROKE: &str = "Stroke Width";
    const SOLID: &str = "Solid";
    const CAP_START: &str = "Start";
    const CAP_END: &str = "End";
    const SHAPE: &str = "Shape";

    pub fn new(vp: VpRef) -> Self {
        let mut inputs = InputGroup::new(vp);

        let selected_id = Rc::new(AtomicI64::new(Self::NO_SELECTION));

        let id_ref = selected_id.clone();
        inputs.add_float_handler(Self::X, None, None, None, move |vp, x| {
            vp.scene.sprite_details(
                id_ref.load(Ordering::Relaxed),
                SpriteDetails {
                    x: Some(x),
                    ..Default::default()
                },
            );
        });

        let id_ref = selected_id.clone();
        inputs.add_float_handler(Self::Y, None, None, None, move |vp, y| {
            vp.scene.sprite_details(
                id_ref.load(Ordering::Relaxed),
                SpriteDetails {
                    y: Some(y),
                    ..Default::default()
                },
            );
        });

        let id_ref = selected_id.clone();
        inputs.add_float_handler(Self::WIDTH, None, None, None, move |vp, w| {
            vp.scene.sprite_details(
                id_ref.load(Ordering::Relaxed),
                SpriteDetails {
                    w: Some(w),
                    ..Default::default()
                },
            );
        });

        let id_ref = selected_id.clone();
        inputs.add_float_handler(Self::HEIGHT, None, None, None, move |vp, h| {
            vp.scene.sprite_details(
                id_ref.load(Ordering::Relaxed),
                SpriteDetails {
                    h: Some(h),
                    ..Default::default()
                },
            );
        });

        inputs.add_line();

        let id_ref = selected_id.clone();
        inputs.add_colour_handler(Self::COLOUR, move |vp, colour| {
            vp.scene.sprite_details(
                id_ref.load(Ordering::Relaxed),
                SpriteDetails {
                    colour: Some(colour),
                    ..Default::default()
                },
            );
        });

        inputs.add_line();

        let id_ref = selected_id.clone();
        inputs.add_float_handler(Self::STROKE, Some(0), None, Some(0.1), move |vp, stroke| {
            vp.scene.sprite_details(
                id_ref.load(Ordering::Relaxed),
                SpriteDetails {
                    stroke: Some(stroke),
                    ..Default::default()
                },
            );
        });

        let id_ref = selected_id.clone();
        inputs.add_checkbox_handler(Self::SOLID, move |vp, solid| {
            vp.scene.sprite_details(
                id_ref.load(Ordering::Relaxed),
                SpriteDetails {
                    solid: Some(solid),
                    ..Default::default()
                },
            );
        });

        inputs.add_line();

        let id_ref = selected_id.clone();
        inputs.add_select_handler(Self::CAP_START, super::CAP_OPTIONS, move |vp, name| {
            vp.scene.sprite_details(
                id_ref.load(Ordering::Relaxed),
                SpriteDetails {
                    cap_start: Some(scene::Cap::from(&name)),
                    ..Default::default()
                },
            )
        });

        let id_ref = selected_id.clone();
        inputs.add_select_handler(Self::CAP_END, super::CAP_OPTIONS, move |vp, name| {
            vp.scene.sprite_details(
                id_ref.load(Ordering::Relaxed),
                SpriteDetails {
                    cap_end: Some(scene::Cap::from(&name)),
                    ..Default::default()
                },
            )
        });

        let id_ref = selected_id.clone();
        inputs.add_select_handler(
            Self::SHAPE,
            &[
                ("Rectangle", "rectangle"),
                ("Ellipse", "ellipse"),
                ("Hexagon", "hexagon"),
            ],
            move |vp, name| {
                vp.scene.sprite_details(
                    id_ref.load(Ordering::Relaxed),
                    SpriteDetails {
                        shape: Some(scene::Shape::from(&name)),
                        ..Default::default()
                    },
                )
            },
        );

        SpriteMenu {
            inputs,
            selected_id,
        }
    }

    pub fn root(&self) -> &Element {
        &self.inputs.root
    }

    pub fn set_sprite_info(&mut self, details: Option<SpriteDetails>) {
        let id = if let Some(details) = details {
            self.inputs.set_or_clear_float(Self::X, details.x);
            self.inputs.set_or_clear_float(Self::Y, details.y);
            self.inputs.set_or_clear_float(Self::WIDTH, details.w);
            self.inputs.set_or_clear_float(Self::HEIGHT, details.h);
            self.inputs
                .set_or_clear_colour(Self::COLOUR, details.colour);
            self.inputs.set_or_clear_float(Self::STROKE, details.stroke);
            self.inputs.set_or_clear_bool(Self::SOLID, details.solid);
            self.inputs
                .set_or_clear_string(Self::CAP_START, details.cap_start.map(|c| c.to_str()));
            self.inputs
                .set_or_clear_string(Self::CAP_END, details.cap_end.map(|c| c.to_str()));
            self.inputs
                .set_or_clear_string(Self::SHAPE, details.shape.map(|c| c.to_str()));
            details.id
        } else {
            Self::NO_SELECTION
        };
        self.selected_id.store(id, Ordering::Relaxed);
    }
}
