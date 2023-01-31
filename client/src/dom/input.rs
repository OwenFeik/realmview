use std::{
    collections::HashMap,
    rc::Rc,
    sync::atomic::{AtomicBool, Ordering},
};

use super::{element::Element, icon::Icon};

pub struct InputGroup {
    pub root: Element,
    changed: Rc<AtomicBool>,
    line: Element,
    inputs: HashMap<String, Element>,
}

impl InputGroup {
    pub fn new() -> InputGroup {
        let root = Element::default().with_class("p-2");
        let line = input_group();
        root.append_child(&line);
        InputGroup {
            root,
            changed: Rc::new(AtomicBool::new(false)),
            line,
            inputs: HashMap::new(),
        }
    }

    pub fn handle_change(&self) -> bool {
        self.changed.swap(false, Ordering::Relaxed)
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

    fn add_input(&mut self, key: &str, mut el: Element) {
        let changed = self.changed.clone();
        el.set_oninput(Box::new(move |_| {
            changed.store(true, Ordering::Relaxed);
        }));
        self.inputs.insert(key.to_string(), el);
    }

    fn add_entry(&mut self, key: &str, el: Element) {
        self.line.append_child(&text(key));
        self.line.append_child(&el);
        self.add_input(key, el);
    }

    pub fn add_float(&mut self, key: &str, min: Option<i32>, max: Option<i32>) {
        self.add_entry(key, float(min, max));
    }

    pub fn add_select(&mut self, key: &str, options: &[(&str, &str)]) {
        self.add_entry(key, select(options));
    }

    pub fn add_checkbox(&mut self, key: &str) {
        let el = Element::new("div").with_class("input-group-text");
        self.line.append_child(&text(key));
        self.line.append_child(&el);
        let input = el
            .child("div")
            .with_class("form-check")
            .child("input")
            .with_class("form-check-input")
            .with_attr("type", "checkbox");
        self.add_input(key, input);
    }

    pub fn add_toggle(&mut self, key: &str, mut icon: Icon) {
        let mut el = Element::new("button")
            .with_classes(&["btn", "btn-sm", "btn-outline-primary"])
            .with_attr("type", "button");
        self.line.append_child(&el);

        // Input element to add to hashmap
        let input = el
            .child("input")
            .with_class("d-none")
            .with_attr("type", "checkbox");

        // Toggle icon and input when clicked
        let i = el.child("i").with_class(&icon.class());
        let input_ref = input.clone();
        el.set_onclick(Box::new(move |_| {
            i.remove_class(&icon.class());
            icon = icon.opposite();
            i.add_class(&icon.class());
            input_ref.toggle_checked();
        }));

        self.add_input(key, input);
    }
}

fn input_group() -> Element {
    Element::default().with_classes(&["input-group", "input-group-sm"])
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
