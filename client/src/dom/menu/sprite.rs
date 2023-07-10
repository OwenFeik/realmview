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

    pub fn new(vp: VpRef) -> Self {
        let mut inputs = InputGroup::new(vp);

        let selected_id = Rc::new(AtomicI64::new(Self::NO_SELECTION));

        let id_ref = selected_id.clone();
        inputs.add_float_handler("X", None, None, None, move |vp, x| {
            vp.scene.sprite_details(
                id_ref.load(Ordering::Relaxed),
                SpriteDetails {
                    x: Some(x),
                    ..Default::default()
                },
            );
        });

        let id_ref = selected_id.clone();
        inputs.add_float_handler("Y", None, None, None, move |vp, y| {
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
        inputs.add_float_handler("H", None, None, None, move |vp, h| {
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
        inputs.add_colour_handler("Colour", move |vp, colour| {
            vp.scene.sprite_details(
                id_ref.load(Ordering::Relaxed),
                SpriteDetails {
                    colour: Some(colour),
                    ..Default::default()
                },
            );
        });

        SpriteMenu {
            inputs,
            selected_id,
        }
    }

    pub fn root(&self) -> &Element {
        &self.inputs.root
    }

    pub fn set_sprite_info(&mut self, details: Option<SpriteDetails>) {
        if let Some(details) = details {
            self.selected_id.store(details.id, Ordering::Relaxed);
            if let Some(x) = details.x {
                self.inputs.set_value_float(Self::X, x);
            }
            if let Some(y) = details.y {
                self.inputs.set_value_float(Self::Y, y);
            }
            if let Some(w) = details.w {
                self.inputs.set_value_float(Self::WIDTH, w);
            }
            if let Some(h) = details.h {
                self.inputs.set_value_float(Self::HEIGHT, h);
            }
        } else {
            self.selected_id
                .store(Self::NO_SELECTION, Ordering::Relaxed);
        }
    }
}
