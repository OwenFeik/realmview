use std::collections::HashMap;

use scene::Colour;
use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;

use super::{element::Element, icon::Icon};
use crate::{start::VpRef, viewport::Viewport};

type Handler = Box<dyn Fn(&mut Viewport)>;
type ValueHandler<T> = Box<dyn Fn(&mut Viewport, T)>;

pub struct InputGroup {
    pub root: Element,
    vp: VpRef,
    line: Element,
    inputs: HashMap<String, Element>,
}

impl InputGroup {
    pub fn new(vp: VpRef) -> InputGroup {
        let root = Element::default();
        let line = input_group();
        root.append_child(&line);
        InputGroup {
            root,
            vp,
            line,
            inputs: HashMap::new(),
        }
    }

    pub fn value_bool(&self, key: &str) -> Option<bool> {
        self.inputs.get(key).map(|e| e.checked())
    }

    pub fn value_string(&self, key: &str) -> Option<String> {
        self.inputs.get(key).map(|e| e.value_string())
    }

    pub fn value_float(&self, key: &str) -> Option<f64> {
        self.inputs.get(key).map(|e| e.value_float())
    }

    pub fn value_f32(&self, key: &str) -> Option<f32> {
        self.value_float(key).map(|v| v.min(f32::MAX as f64) as f32)
    }

    pub fn value_unsigned(&self, key: &str) -> Option<u32> {
        self.value_float(key).map(|v| v as u32)
    }

    pub fn value_colour(&self, key: &str) -> Option<Colour> {
        self.value_string(key).and_then(|hex| hex_to_colour(&hex))
    }

    pub fn set_value_bool(&self, key: &str, value: bool) {
        if let Some(e) = self.inputs.get(key) {
            e.set_checked(value);
        }
    }

    pub fn set_value_string(&self, key: &str, value: &str) {
        if let Some(e) = self.inputs.get(key) {
            e.set_value_string(value);
        }
    }

    pub fn set_value_float(&self, key: &str, value: f32) {
        if let Some(e) = self.inputs.get(key) {
            e.set_value_float(value as f64);
        }
    }

    pub fn set_value_colour(&self, key: &str, value: Colour) {
        if let Some(e) = self.inputs.get(key) {
            e.set_value_string(&colour_to_hex(value));
        }
    }

    pub fn set_options<T: AsRef<str>>(&self, key: &str, options: &[(T, T)]) {
        if let Some(e) = self.inputs.get(key) {
            e.set_options(options);
        }
    }

    pub fn add_line(&mut self) {
        self.line = input_group().with_class("mt-1");
        self.root.append_child(&self.line);
    }

    fn add_input(&mut self, key: &str, el: Element) {
        self.inputs.insert(key.to_string(), el);
    }

    fn add_entry(&mut self, key: &str, el: Element) {
        self.line.append_child(&text(key));
        self.line.append_child(&el);
        self.add_input(key, el);
    }

    pub fn add_string(&mut self, key: &str) {
        self.add_entry(key, string());
    }

    pub fn add_toggle_string(&mut self, key: &str, label: bool, action: ValueHandler<String>) {
        let el = string();
        el.set_enabled(false);

        if label {
            self.add_entry(key, el.clone());
        } else {
            self.line.append_child(&el);
            self.add_input(key, el.clone());
        }

        self.add_toggle(
            &format!("{}_toggle", key),
            Icon::Edit,
            Icon::Ok,
            Box::new(move |vp, enabled| {
                el.set_enabled(enabled);

                // Save value when disabling.
                if !enabled {
                    action(vp, el.value_string());
                }
            }),
        );
    }

    pub fn add_float(&mut self, key: &str, min: Option<i32>, max: Option<i32>) {
        self.add_entry(key, float(min, max));
    }

    pub fn add_float_handler(
        &mut self,
        key: &str,
        min: Option<i32>,
        max: Option<i32>,
        action: ValueHandler<f32>,
    ) {
        self.add_float(key, min, max);
        let el = self.inputs.get_mut(key).unwrap();
        let el_ref = el.clone();
        let vp_ref = self.vp.clone();
        el.set_oninput(Box::new(move |_| {
            action(&mut vp_ref.lock(), el_ref.value_float() as f32);
        }));
    }

    pub fn add_select(&mut self, key: &str, options: &[(&str, &str)]) {
        self.add_entry(key, select(options));
    }

    pub fn add_select_handler(
        &mut self,
        key: &str,
        options: &[(&str, &str)],
        action: ValueHandler<String>,
    ) {
        self.add_select(key, options);
        let el = self.inputs.get_mut(key).unwrap();
        let el_ref = el.clone();
        let vp_ref = self.vp.clone();
        el.set_oninput(Box::new(move |_| {
            action(&mut vp_ref.lock(), el_ref.value_string());
        }));
    }

    pub fn add_checkbox(&mut self, key: &str) {
        let el = Element::new("div").with_class("input-group-text");
        self.line.append_child(&text(key));
        self.line.append_child(&el);
        let mut input = el
            .child("div")
            .with_class("form-check")
            .child("input")
            .with_class("form-check-input")
            .with_attr("type", "checkbox");
        self.add_input(key, input);
    }

    pub fn add_checkbox_handler(&mut self, key: &str, action: ValueHandler<bool>) {
        self.add_checkbox(key);
        let input = self.inputs.get_mut(key).unwrap();
        let input_ref = input.clone();
        let vp_ref = self.vp.clone();
        input.set_oninput(Box::new(move |_| {
            action(&mut vp_ref.lock(), input_ref.checked())
        }));
    }

    pub fn add_toggle(&mut self, key: &str, a: Icon, b: Icon, action: ValueHandler<bool>) {
        let mut el = button();
        self.line.append_child(&el);

        // Input element to add to hashmap
        let input = el
            .child("input")
            .with_class("d-none")
            .with_attr("type", "checkbox");

        // Toggle icon and input when clicked
        let i = el.child("i").with_class(&a.class());
        let input_ref = input.clone();
        let vp_ref = self.vp.clone();
        el.set_onclick(Box::new(move |_| {
            let value = !input_ref.checked(); // Initially true

            let (from, to) = if value { (a, b) } else { (b, a) };
            i.remove_class(&from.class());
            i.add_class(&to.class());
            input_ref.toggle_checked();

            action(&mut vp_ref.lock(), value);
        }));

        self.add_input(key, input);
    }

    pub fn add_button(&mut self, icon: Icon, action: Handler) {
        let mut el = button();
        el.child("i").with_class(&icon.class());

        let vp = self.vp.clone();
        el.set_onclick(Box::new(move |_| action(&mut vp.lock())));

        self.line.append_child(&el);
    }

    pub fn add_radio(&mut self, key: &str, selected: bool, action: Handler) {
        let el = self.line.child("div").with_class("input-group-text");
        let mut input = el
            .child("input")
            .with_classes(&["form-check-input", "mt-0"])
            .with_attrs(&[("name", key), ("type", "radio")]);
        input.set_checked(selected);
        let vp_ref = self.vp.clone();
        input.set_oninput(Box::new(move |_| {
            action(&mut vp_ref.lock());
        }));
    }

    pub fn add_colour(&mut self, key: &str) {
        self.add_entry(key, colour());
    }

    pub fn add_colour_handler(&mut self, key: &str, action: ValueHandler<Colour>) {
        self.add_colour(key);
        let mut input = self.inputs.get_mut(key).unwrap();
        let vp_ref = self.vp.clone();
        input.set_oninput(Box::new(move |evt| {
            evt.target().map(|target| {
                let hex = target.unchecked_ref::<HtmlInputElement>().value();
                if let Some(colour) = hex_to_colour(&hex) {
                    action(&mut vp_ref.lock(), colour);
                }
            });
        }));
    }

    pub fn add_icon_radio_handler(&mut self, key: &str, icons: &[Icon], action: ValueHandler<u32>) {
        let el = self
            .line
            .child("div")
            .with_class("btn-group")
            .with_attr("role", "group");

        let action_ref = std::rc::Rc::new(action);

        let name = format!("{key}_radio");
        for (i, icon) in icons.iter().enumerate() {
            let id = format!("{key}_option_{i}");
            let mut input = el
                .child("input")
                .with_attrs(&[("id", &id), ("type", "radio"), ("name", &name)])
                .with_class("btn-check");
            el.child("label")
                .with_attr("for", &id)
                .with_classes(&["btn", "btn-sm", "btn-outline-primary"])
                .with_child(&icon.element());
            let vp_ref = self.vp.clone();
            let action_ref = action_ref.clone();
            input.set_oninput(Box::new(move |_| action_ref(&mut vp_ref.lock(), i as u32)));
        }
    }
}

fn input_group() -> Element {
    Element::default().with_classes(&["input-group", "input-group-sm"])
}

fn string() -> Element {
    Element::input()
        .with_class("form-control")
        .with_attr("maxlength", "256")
        .with_attr("type", "text")
}

fn float(min: Option<i32>, max: Option<i32>) -> Element {
    let el = Element::input()
        .with_class("form-control")
        .with_attrs(&[("type", "number"), ("autocomplete", "off")]);

    if let Some(min) = min {
        el.set_attr("min", &min.to_string());
    }

    if let Some(max) = max {
        el.set_attr("max", &max.to_string());
    }

    el
}

fn text(text: &str) -> Element {
    Element::span()
        .with_class("input-group-text")
        .with_text(text)
}

fn select(options: &[(&str, &str)]) -> Element {
    let el = Element::new("select");
    el.add_class("form-select");
    el.set_options(options);
    el
}

fn button() -> Element {
    Element::new("button")
        .with_classes(&["btn", "btn-sm", "btn-outline-primary"])
        .with_attr("type", "button")
}

fn colour() -> Element {
    Element::input()
        .with_class("form-control")
        .with_attr("type", "color")
}

fn colour_to_hex(colour: Colour) -> String {
    format!(
        "#{:02X}{:02X}{:02X}",
        (colour.r() * 255.0) as u32,
        (colour.g() * 255.0) as u32,
        (colour.b() * 255.0) as u32
    )
}

fn hex_to_num(hex: &str) -> Option<f32> {
    const RADIX: u32 = 16;
    i32::from_str_radix(hex, RADIX)
        .ok()
        .map(|int| (int as f32) / 255.0)
}

fn hex_to_colour(hex: &str) -> Option<Colour> {
    const ALPHA: f32 = 1.0;
    match hex.len() {
        6 => Some(Colour([
            hex_to_num(&hex[0..=1])?,
            hex_to_num(&hex[2..=3])?,
            hex_to_num(&hex[4..=5])?,
            ALPHA,
        ])),
        7 => hex_to_colour(&hex[1..]),
        _ => None,
    }
}
