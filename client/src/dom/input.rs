use std::collections::HashMap;

use crate::dom::element::Element;

pub struct InputGroup {
    pub root: Element,
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

    pub fn add_line(&mut self) {
        self.line = input_group().with_class("mt-1");
        self.root.append_child(&self.line);
    }

    fn add_input(&mut self, key: &str, el: Element) {
        self.line.append_child(&text(key));
        self.line.append_child(&el);
        self.inputs.insert(key.to_string(), el);
    }

    pub fn add_float(&mut self, key: &str, min: Option<i32>, max: Option<i32>) {
        self.add_input(key, float(min, max));
    }

    pub fn add_select(&mut self, key: &str, options: &[(&str, &str)]) {
        self.add_input(key, select(options));
    }

    pub fn add_bool(&mut self, key: &str) {
        let el = Element::new("div").with_class("input-group-text");
        self.line.append_child(&text(key));
        self.line.append_child(&el);
        let input = el
            .child("div")
            .with_class("form-check")
            .child("input")
            .with_class("form-check-input")
            .with_attr("type", "checkbox");
        self.inputs.insert(key.to_string(), input);
    }
}

fn input_group() -> Element {
    Element::default()
        .with_class("input-group")
        .with_class("input-group-sm")
}

fn float(min: Option<i32>, max: Option<i32>) -> Element {
    let el = Element::input()
        .with_class("form-control")
        .with_attr("type", "number")
        .with_attr("autocomplete", "off");

    if let Some(min) = min {
        el.set_attr("min", &min.to_string());
    }

    if let Some(max) = max {
        el.set_attr("max", &max.to_string());
    }

    el
}

fn text(text: &str) -> Element {
    let el = Element::span();
    el.add_class("input-group-text");
    el.set_text(text);
    el
}

fn select(options: &[(&str, &str)]) -> Element {
    let el = Element::new("select");
    el.add_class("form-select");
    el.set_options(options);
    el
}
