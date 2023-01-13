use anyhow::anyhow;
use wasm_bindgen::{prelude::Closure, JsCast};

use super::{create_element, get_body, get_element_by_id};
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
        match create_element(name)?.dyn_into::<web_sys::HtmlElement>() {
            Ok(e) => Ok(Element {
                element: e,
                listener: None,
            }),
            Err(_) => Err(anyhow!("Couldn't cast to HtmlElement.")),
        }
    }

    pub fn add_to_page(&self) {
        get_body()
            .expect("Missing document.")
            .append_child(self.node())
            .ok();
    }

    pub fn by_id(id: &str) -> Option<Element> {
        get_element_by_id(id).ok().map(|element| Self {
            element,
            listener: None,
        })
    }

    pub fn anchor() -> Self {
        Self::new("a")
    }

    pub fn list() -> Self {
        Self::new("ul")
    }

    pub fn item() -> Self {
        Self::new("li")
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

    pub fn set_text(&self, text: &str) {
        self.element.set_inner_text(text);
    }

    pub fn append_child(&self, child: &Element) {
        self.element.append_child(child.node()).ok();
    }

    pub fn set_onclick(&mut self, handler: Box<dyn FnMut(web_sys::Event)>) {
        self.add_event_listener("click", handler);
    }

    pub fn set_pos(&self, pos: ViewportPoint) {
        self.set_css("left", &format!("{}px", pos.x));
        self.set_css("top", &format!("{}px", pos.y));
    }

    fn add_event_listener(&mut self, on: &str, handler: Box<dyn FnMut(web_sys::Event)>) {
        let closure = Closure::wrap(handler);
        self.element
            .add_event_listener_with_callback(on, closure.as_ref().unchecked_ref())
            .ok();
        self.listener = Some(closure);
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
