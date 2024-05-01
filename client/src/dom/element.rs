use wasm_bindgen::{prelude::Closure, JsCast};
use web_sys::{HtmlElement, HtmlInputElement};

use super::icon::Icon;
use crate::bridge::{get_body, get_document};
use crate::viewport::ViewportPoint;
use crate::Res;

pub struct Element {
    element: web_sys::HtmlElement,
}

impl Element {
    pub fn new(name: &str) -> Self {
        Self::try_new(name).expect("Failed to create an element.")
    }

    pub fn try_new(name: &str) -> Res<Element> {
        let element = get_document()?
            .create_element(name)
            .map(|e| e.unchecked_into::<HtmlElement>())
            .map_err(|e| format!("Element creation failed: {e:?}."))?;

        Ok(Element { element })
    }

    pub fn raw(self) -> HtmlElement {
        self.element
    }

    pub fn on_page(self) -> Self {
        self.add_to_page();
        self
    }

    pub fn add_to_page(&self) {
        get_body()
            .expect("Missing document.")
            .append_child(self.node())
            .ok();
    }

    pub fn by_id(id: &str) -> Option<Element> {
        get_document().ok()?.get_element_by_id(id).map(|e| Self {
            element: e.unchecked_into::<HtmlElement>(),
        })
    }

    pub fn by_selector(selector: &str) -> Option<Element> {
        get_document()
            .ok()?
            .query_selector(selector)
            .ok()?
            .map(Element::from)
    }

    pub fn anchor() -> Self {
        Self::new("a")
    }

    pub fn input() -> Self {
        Self::new("input")
    }

    pub fn item() -> Self {
        Self::new("li")
    }

    pub fn list() -> Self {
        Self::new("ul")
    }

    pub fn span() -> Self {
        Self::new("span")
    }

    pub fn node(&self) -> &web_sys::Node {
        self.element.unchecked_ref::<web_sys::Node>()
    }

    pub fn remove(&self) {
        self.element.remove();
    }

    pub fn add_class(&self, class: &str) {
        self.element.class_list().add_1(class).ok();
    }

    pub fn remove_class(&self, class: &str) {
        self.element.class_list().remove_1(class).ok();
    }

    pub fn add_classes(&self, classes: &[&str]) {
        classes.iter().for_each(|class| self.add_class(class))
    }

    pub fn with_class(self, class: &str) -> Self {
        self.add_class(class);
        self
    }

    pub fn with_classes(self, classes: &[&str]) -> Self {
        self.add_classes(classes);
        self
    }

    pub fn has_class(&self, class: &str) -> bool {
        self.element.class_list().contains(class)
    }

    pub fn set_css(&self, property: &str, value: &str) {
        self.try_set_css(property, value).ok();
    }

    pub fn hide(&self) {
        self.add_class("d-none");
    }

    pub fn show(&self) {
        self.remove_class("d-none");
    }

    pub fn get_attr(&self, name: &str) -> Option<String> {
        self.element.get_attribute(name)
    }

    pub fn set_attr(&self, name: &str, value: &str) {
        self.try_set_attr(name, value).ok();
    }

    pub fn set_attrs(&self, attrs: &[(&str, &str)]) {
        for (name, value) in attrs {
            self.set_attr(name, value);
        }
    }

    pub fn with_attr(self, name: &str, value: &str) -> Self {
        self.set_attr(name, value);
        self
    }

    pub fn with_attrs(self, attrs: &[(&str, &str)]) -> Self {
        self.set_attrs(attrs);
        self
    }

    pub fn set_text(&self, text: &str) {
        self.element.set_inner_text(text);
    }

    pub fn with_text(self, text: &str) -> Self {
        self.set_text(text);
        self
    }

    pub fn append_child(&self, child: &Element) {
        self.element.append_child(child.node()).ok();
    }

    pub fn child(&self, name: &str) -> Element {
        let el = Element::new(name);
        self.append_child(&el);
        el
    }

    pub fn with_child(self, child: &Element) -> Self {
        self.append_child(child);
        self
    }

    pub fn icon(&self, icon: Icon) -> Element {
        self.child("i").with_class(&icon.class())
    }

    pub fn set_onclick(&mut self, handler: Box<dyn FnMut(web_sys::Event)>) {
        self.add_event_listener("click", handler);
    }

    pub fn set_oninput(&mut self, handler: Box<dyn FnMut(web_sys::Event)>) {
        self.add_event_listener("input", handler);
    }

    pub fn set_pos(&self, pos: ViewportPoint) {
        self.set_css("left", &format!("{}px", pos.x));
        self.set_css("top", &format!("{}px", pos.y));
    }

    pub fn checked(&self) -> bool {
        self.as_input().checked()
    }

    pub fn value_string(&self) -> String {
        self.as_input().value()
    }

    pub fn value_float(&self) -> f64 {
        self.as_input().value_as_number()
    }

    pub fn set_checked(&self, value: bool) {
        self.as_input().set_checked(value);
    }

    pub fn toggle_checked(&self) {
        self.set_checked(!self.checked());
    }

    pub fn set_value_string(&self, value: &str) {
        self.as_input().set_value(value);
    }

    pub fn set_value_float(&self, value: f32) {
        self.as_input().set_value_as_number(value as f64);
    }

    pub fn clear_value(&self) {
        self.as_input().set_value("");
    }

    pub fn set_inner_html(&self, inner_html: &str) {
        self.element.set_inner_html(inner_html);
    }

    pub fn clear(&self) {
        self.set_inner_html("");
    }

    /// Replace body of this element (assumed a select input) with a list of
    /// option elements specified by the (label, value) pairs provided.
    pub fn set_options<T: AsRef<str>>(&self, options: &[(T, T)]) {
        self.set_inner_html("");
        for (key, value) in options {
            let option = self.child("option");
            option.set_text(key.as_ref());
            option.set_value_string(value.as_ref());
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.as_input().set_disabled(!enabled);
    }

    pub fn click(&self) {
        self.element.click();
    }

    pub fn event(&self, event: &str) {
        if let Ok(event) = web_sys::Event::new(event) {
            self.element.dispatch_event(&event).ok();
        }
    }

    fn add_event_listener(&mut self, on: &str, handler: Box<dyn FnMut(web_sys::Event)>) {
        let closure = Closure::wrap(handler);
        self.element
            .add_event_listener_with_callback(on, closure.as_ref().unchecked_ref())
            .ok();

        // Memory leak. Thought I could work around this by holding a reference
        // in the Element, but that means that if the Element is dropped the
        // closure is dropped, which is annoying when chaining with `.child`.
        // Could maybe work around by having the parent store its children but
        // that's probably more annoying than a little memory leak.
        closure.forget();
    }

    fn as_input(&self) -> &HtmlInputElement {
        self.element.unchecked_ref::<HtmlInputElement>()
    }

    fn try_set_css(&self, property: &str, value: &str) -> Res<()> {
        self.element
            .style()
            .set_property(property, value)
            .map_err(|e| format!("Failed to set element CSS: {e:?}."))
    }

    fn try_set_attr(&self, name: &str, value: &str) -> Res<()> {
        self.element
            .set_attribute(name, value)
            .map_err(|e| format!("Failed to set element attribute: {e:?}."))
    }
}

impl Clone for Element {
    fn clone(&self) -> Self {
        Self {
            element: self.element.clone(),
        }
    }
}

impl Default for Element {
    fn default() -> Self {
        Self::new("div")
    }
}

impl From<web_sys::Element> for Element {
    fn from(value: web_sys::Element) -> Self {
        Element {
            element: value.unchecked_into::<web_sys::HtmlElement>(),
        }
    }
}

impl From<web_sys::EventTarget> for Element {
    fn from(value: web_sys::EventTarget) -> Self {
        Element::from(value.unchecked_into::<web_sys::Element>())
    }
}
