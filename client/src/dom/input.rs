use std::collections::HashMap;

use super::{element::Element, icon::Icon};
use crate::{start::VpRef, viewport::Viewport};

type Handler<T> = Box<dyn Fn(&mut Viewport, T)>;

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

    pub fn value_unsigned(&self, key: &str) -> Option<u32> {
        self.value_float(key).map(|v| v as u32)
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

    pub fn set_value_float(&self, key: &str, value: f64) {
        if let Some(e) = self.inputs.get(key) {
            e.set_value_float(value);
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

    pub fn add_toggle_string(
        &mut self,
        key: &str,
        label: bool,
        action: Box<dyn Fn(&mut Viewport, String)>,
    ) {
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

    pub fn add_float(
        &mut self,
        key: &str,
        min: Option<i32>,
        max: Option<i32>,
        action: Box<dyn Fn(&mut Viewport, f32)>,
    ) {
        let mut el = float(min, max);
        let el_ref = el.clone();
        let vp_ref = self.vp.clone();
        el.set_oninput(Box::new(move |_| {
            action(&mut vp_ref.lock(), el_ref.value_float() as f32);
        }));
        self.add_entry(key, el);
    }

    pub fn add_select(
        &mut self,
        key: &str,
        options: &[(&str, &str)],
        action: Box<dyn Fn(&mut Viewport, String)>,
    ) {
        let mut el = select(options);
        let el_ref = el.clone();
        let vp_ref = self.vp.clone();
        el.set_oninput(Box::new(move |_| {
            action(&mut vp_ref.lock(), el_ref.value_string());
        }));
        self.add_entry(key, el);
    }

    pub fn add_checkbox(&mut self, key: &str, action: Handler<bool>) {
        let el = Element::new("div").with_class("input-group-text");
        self.line.append_child(&text(key));
        self.line.append_child(&el);
        let mut input = el
            .child("div")
            .with_class("form-check")
            .child("input")
            .with_class("form-check-input")
            .with_attr("type", "checkbox");
        let input_ref = input.clone();
        let vp_ref = self.vp.clone();
        input.set_oninput(Box::new(move |_| {
            action(&mut vp_ref.lock(), input_ref.checked())
        }));
        self.add_input(key, input);
    }

    pub fn add_toggle(&mut self, key: &str, a: Icon, b: Icon, action: Handler<bool>) {
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

    pub fn add_button(&mut self, icon: Icon, action: Box<dyn Fn(&mut Viewport)>) {
        let mut el = button();
        el.child("i").with_class(&icon.class());

        let vp = self.vp.clone();
        el.set_onclick(Box::new(move |_| action(&mut vp.lock())));

        self.line.append_child(&el);
    }

    pub fn add_radio(&mut self, key: &str, selected: bool, action: Box<dyn Fn(VpRef)>) {
        let el = self.line.child("div").with_class("input-group-text");
        let mut input = el
            .child("input")
            .with_classes(&["form-check-input", "mt-0"])
            .with_attrs(&[("name", key), ("type", "radio")]);
        input.set_checked(selected);
        let vp_ref = self.vp.clone();
        input.set_oninput(Box::new(move |_| {
            action(vp_ref.clone());
        }));
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
