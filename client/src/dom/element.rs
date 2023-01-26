use anyhow::anyhow;
use wasm_bindgen::{prelude::Closure, JsCast};
use web_sys::{HtmlElement, HtmlInputElement};

use crate::bridge::{get_body, get_document};
use crate::viewport::ViewportPoint;

pub struct Element {
    pub element: web_sys::HtmlElement,
    listener: Option<Closure<dyn FnMut(web_sys::Event)>>,
}

impl Element {
    pub fn new(name: &str) -> Self {
        Self::try_new(name).expect("Failed to create an element.")
    }

    pub fn try_new(name: &str) -> anyhow::Result<Element> {
        let element = get_document()?
            .create_element(name)
            .map(|e| e.unchecked_into::<HtmlElement>())
            .map_err(|e| anyhow!("Element creation failed: {e:?}."))?;

        Ok(Element {
            element,
            listener: None,
        })
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
            listener: None,
        })
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

    pub fn with_class(self, class: &str) -> Self {
        self.add_class(class);
        self
    }

    pub fn has_class(&self, class: &str) -> bool {
        self.element.class_list().contains(class)
    }

    pub fn set_css(&self, property: &str, value: &str) {
        self.try_set_css(property, value).ok();
    }

    pub fn try_set_css(&self, property: &str, value: &str) -> anyhow::Result<()> {
        self.element
            .style()
            .set_property(property, value)
            .map_err(|e| anyhow!("Failed to set element CSS: {e:?}."))
    }

    pub fn hide(&self) {
        self.set_css("display", "none");
    }

    pub fn show(&self) {
        self.set_css("display", "");
    }

    pub fn set_attr(&self, name: &str, value: &str) {
        self.try_set_attr(name, value).ok();
    }

    pub fn try_set_attr(&self, name: &str, value: &str) -> anyhow::Result<()> {
        self.element
            .set_attribute(name, value)
            .map_err(|e| anyhow!("Failed to set element attribute: {e:?}."))
    }

    pub fn with_attr(self, name: &str, value: &str) -> Self {
        self.set_attr(name, value);
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

    pub fn set_onclick(&mut self, handler: Box<dyn FnMut(web_sys::Event)>) {
        self.add_event_listener("click", handler);
    }

    pub fn set_pos(&self, pos: ViewportPoint) {
        self.set_css("left", &format!("{}px", pos.x));
        self.set_css("top", &format!("{}px", pos.y));
    }

    pub fn value_string(&self) -> String {
        self.as_input().value()
    }

    pub fn set_value_string(&self, value: &str) {
        self.as_input().set_value(value);
    }

    pub fn value_float(&self) -> f64 {
        self.as_input().value_as_number()
    }

    pub fn set_value_float(&self, value: f64) {
        self.as_input().set_value_as_number(value);
    }

    pub fn set_inner_html(&self, inner_html: &str) {
        self.element.set_inner_html(inner_html);
    }

    fn add_event_listener(&mut self, on: &str, handler: Box<dyn FnMut(web_sys::Event)>) {
        let closure = Closure::wrap(handler);
        self.element
            .add_event_listener_with_callback(on, closure.as_ref().unchecked_ref())
            .ok();
        self.listener = Some(closure);
    }

    fn as_input(&self) -> &HtmlInputElement {
        self.element.unchecked_ref::<HtmlInputElement>()
    }
}

impl Clone for Element {
    fn clone(&self) -> Self {
        Self {
            element: self.element.clone(),
            listener: None,
        }
    }
}

impl Default for Element {
    fn default() -> Self {
        Self::new("div")
    }
}
