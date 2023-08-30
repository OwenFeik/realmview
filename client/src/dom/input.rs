use std::collections::HashMap;

use scene::Colour;

use super::{element::Element, icon::Icon};
use crate::{bridge::log, start::VpRef, viewport::Viewport};

pub trait Handler = Fn(&mut Viewport) + 'static;
pub trait ValueHandler<T> = Fn(&mut Viewport, T) + 'static;

pub struct InputGroup {
    root: Element,
    vp: VpRef,
    line: Element,
    inputs: HashMap<String, Element>,
}

impl InputGroup {
    const OPACITY_ATTR: &'static str = "data-opacity";

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

    pub fn root(&self) -> &Element {
        &self.root
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.inputs.get(key).map(|e| e.checked())
    }

    pub fn get_string(&self, key: &str) -> Option<String> {
        self.inputs.get(key).map(|e| e.value_string())
    }

    pub fn get_f64(&self, key: &str) -> Option<f64> {
        self.inputs.get(key).map(|e| e.value_float())
    }

    pub fn get_f32(&self, key: &str) -> Option<f32> {
        self.get_f64(key).map(|v| v.min(f32::MAX as f64) as f32)
    }

    pub fn get_u32(&self, key: &str) -> Option<u32> {
        self.get_f64(key).map(|v| v as u32)
    }

    pub fn get_colour(&self, key: &str) -> Option<Colour> {
        self.inputs.get(key).and_then(Self::colour_input_value)
    }

    pub fn clear(&self, key: &str) {
        if let Some(input) = self.inputs.get(key) {
            input.clear_value();
        }
    }

    pub fn set_bool(&self, key: &str, value: bool) {
        if let Some(e) = self.inputs.get(key) {
            e.set_checked(value);
        }
    }

    pub fn set_or_clear_bool(&self, key: &str, value: Option<bool>) {
        if let Some(value) = value {
            self.set_bool(key, value);
        } else {
            self.set_bool(key, false);
        }
    }

    pub fn set_string(&self, key: &str, value: &str) {
        if let Some(e) = self.inputs.get(key) {
            e.set_value_string(value);
        }
    }

    pub fn set_or_clear_string<T>(&self, key: &str, value: Option<T>)
    where
        T: AsRef<str>,
    {
        if let Some(value) = value {
            self.set_string(key, value.as_ref());
        } else {
            self.clear(key);
        }
    }

    pub fn set_float(&self, key: &str, value: f32) {
        if let Some(number_input) = self.inputs.get(key) {
            // Round to 2 decimal places for display.
            number_input.set_value_string(&format!("{:.2}", value));
        }
    }

    pub fn set_or_clear_float(&self, key: &str, value: Option<f32>) {
        if let Some(value) = value {
            self.set_float(key, value);
        } else {
            self.clear(key);
        }
    }

    pub fn set_colour(&self, key: &str, value: Colour) {
        if let Some(colour_input) = self.inputs.get(key) {
            let opacity = (value.a() * 100.0).round();

            colour_input.set_value_string(&colour_to_hex(value));
            colour_input.set_attr(Self::OPACITY_ATTR, &opacity.to_string());

            // Need to find and update the opacity input as it is a separate
            // element. Order is [colour_input, label, opacity_input] so we can
            // use colour_input.next_element_sibling.next_element_sibling.
            if let Some(opacity_input) = colour_input
                .clone()
                .raw()
                .next_element_sibling()
                .as_ref()
                .and_then(web_sys::Element::next_element_sibling)
            {
                Element::from(opacity_input).set_value_float(opacity);
            }
        }
    }

    pub fn set_or_clear_colour(&self, key: &str, value: Option<Colour>) {
        if let Some(value) = value {
            self.set_colour(key, value);
        } else {
            self.clear(key);
        }
    }

    pub fn set_selected_icon_radio(&self, key: &str, icon: Icon) {
        if let Some(radio) = Element::by_id(&Self::icon_radio_input_id(key, icon)) {
            radio.set_checked(true);
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

    fn set_input_handler<H: Fn(&mut Viewport, Element) + 'static>(&mut self, key: &str, action: H) {
        if let Some(element) = self.inputs.get_mut(key) {
            let vp_ref = self.vp.clone();
            element.set_oninput(Box::new(move |event: web_sys::Event| {
                if let Some(el) = event.target().map(Element::from) {
                    if let Ok(mut lock) = vp_ref.try_lock() {
                        action(&mut lock, el);
                    } else {
                        log("Failed to lock viewport for input event.");
                    }
                }
            }));
        }
    }

    pub fn add_string(&mut self, key: &str) {
        self.add_entry(key, string());
    }

    pub fn add_toggle_string<H: ValueHandler<String>>(
        &mut self,
        key: &str,
        label: bool,
        action: H,
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
            move |vp, enabled| {
                el.set_enabled(enabled);

                // Save value when disabling.
                if !enabled {
                    action(vp, el.value_string());
                }
            },
        );
    }

    pub fn add_float(&mut self, key: &str, min: Option<i32>, max: Option<i32>, step: Option<f32>) {
        self.add_entry(key, float(min, max, step));
    }

    pub fn add_float_handler<H: ValueHandler<f32>>(
        &mut self,
        key: &str,
        min: Option<i32>,
        max: Option<i32>,
        step: Option<f32>,
        action: H,
    ) {
        self.add_float(key, min, max, step);
        self.set_input_handler(key, move |vp, el| action(vp, el.value_float() as f32));
    }

    pub fn add_select(&mut self, key: &str, options: &[(&str, &str)]) {
        self.add_entry(key, select(options));
    }

    pub fn add_select_handler<H: ValueHandler<String>>(
        &mut self,
        key: &str,
        options: &[(&str, &str)],
        action: H,
    ) {
        self.add_select(key, options);
        self.set_input_handler(key, move |vp, el| action(vp, el.value_string()));
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

    pub fn add_checkbox_handler<H: ValueHandler<bool>>(&mut self, key: &str, action: H) {
        self.add_checkbox(key);
        self.set_input_handler(key, move |vp, el| action(vp, el.checked()));
    }

    pub fn add_toggle<H: ValueHandler<bool>>(&mut self, key: &str, a: Icon, b: Icon, action: H) {
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
            if let Ok(mut lock) = vp_ref.try_lock() {
                let value = !input_ref.checked(); // Initially true

                let (from, to) = if value { (a, b) } else { (b, a) };
                i.remove_class(&from.class());
                i.add_class(&to.class());
                input_ref.toggle_checked();

                action(&mut lock, value);
            } else {
                log("Failed to lock viewport for toggle button click.");
            }
        }));

        self.add_input(key, input);
    }

    pub fn add_button<H: Handler>(&mut self, icon: Icon, action: H) {
        let mut el = button();
        el.child("i").with_class(&icon.class());

        let vp = self.vp.clone();
        el.set_onclick(Box::new(move |_| {
            if let Ok(mut lock) = vp.try_lock() {
                action(&mut lock);
            } else {
                log("Failed to lock viewport for button click.");
            }
        }));

        self.line.append_child(&el);
    }

    pub fn add_radio<H: Handler>(&mut self, key: &str, selected: bool, action: H) {
        let el = self.line.child("div").with_class("input-group-text");
        let input = el
            .child("input")
            .with_classes(&["form-check-input", "mt-0"])
            .with_attrs(&[("name", key), ("type", "radio")]);
        input.set_checked(selected);
        self.set_input_handler(key, move |vp, _| action(vp));
    }

    fn colour_input_opacity(input: &Element) -> f32 {
        input
            .get_attr(Self::OPACITY_ATTR)
            .and_then(|v| v.parse().ok())
            .map(|v: f32| v / 100.0)
            .unwrap_or(1.0)
    }

    fn colour_input_value(input: &Element) -> Option<Colour> {
        let hex = input.value_string();
        let opacity = Self::colour_input_opacity(input);
        hex_to_colour(&hex, opacity)
    }

    pub fn add_colour(&mut self, key: &str) {
        let mut colour = Element::input()
            .with_class("form-control")
            .with_attr("type", "color")
            .with_attr(Self::OPACITY_ATTR, "100");

        // Opacity is handled with a separate float input with sets an
        // attribute on the colour input when its value changes and listens for
        // changes on the colour input to handle system writes.
        let mut opacity = float(Some(0), Some(100), Some(10.0));
        let colour_ref = colour.clone();
        opacity.set_oninput(Box::new(move |evt| {
            if let Some(input) = evt.target() {
                let opacity = Element::from(input).value_string();
                colour_ref.set_attr(Self::OPACITY_ATTR, &opacity);

                // Trigger colour input to handle changed opacity.
                colour_ref.event("input");
            }
        }));

        let opacity_ref = opacity.clone();
        colour.set_oninput(Box::new(move |evt| {
            if let Some(target) = evt.target() {
                let opacity = Self::colour_input_opacity(&Element::from(target));
                opacity_ref.set_value_float((opacity * 100.0).round());
            }
        }));

        self.add_entry(key, colour);
        self.line.append_child(&text("Opacity"));
        self.line.append_child(&opacity);
    }

    pub fn add_colour_handler<H: ValueHandler<Colour>>(&mut self, key: &str, action: H) {
        self.add_colour(key);
        self.set_input_handler(key, move |vp, el| {
            if let Some(colour) = Self::colour_input_value(&el) {
                action(vp, colour);
            }
        });
    }

    fn icon_radio_input_id(key: &str, icon: Icon) -> String {
        format!("{key}_option_{}", icon.class())
    }

    pub fn add_icon_radio_handler<H: ValueHandler<Icon>>(
        &mut self,
        key: &str,
        icons: &[Icon],
        action: H,
    ) {
        let el = self
            .line
            .child("div")
            .with_class("btn-group")
            .with_attr("role", "group");

        let action_ref = std::rc::Rc::new(action);

        let name = format!("{key}_radio");
        for &icon in icons {
            let id = Self::icon_radio_input_id(key, icon);
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
            input.set_oninput(Box::new(move |_| {
                if let Ok(mut lock) = vp_ref.lock() {
                    action_ref(&mut lock, icon);
                } else {
                    log("Failed to lock viewport for icon radio input.");
                }
            }));
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

fn float(min: Option<i32>, max: Option<i32>, step: Option<f32>) -> Element {
    let el = Element::input()
        .with_class("form-control")
        .with_attrs(&[("type", "number"), ("autocomplete", "off")]);

    if let Some(step) = step {
        el.set_attr("step", &step.to_string())
    }

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

fn hex_to_colour(hex: &str, opacity: f32) -> Option<Colour> {
    match hex.len() {
        6 => Some(Colour([
            hex_to_num(&hex[0..=1])?,
            hex_to_num(&hex[2..=3])?,
            hex_to_num(&hex[4..=5])?,
            opacity,
        ])),
        7 => hex_to_colour(&hex[1..], opacity),
        _ => None,
    }
}
