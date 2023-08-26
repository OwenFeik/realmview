use crate::{dom::element::Element, viewport::ViewportPoint};

const HOVER_ROOT_ID: &str = "canvas_text";

struct HoverText {
    element: Element,
}

impl HoverText {
    const HOVER_TEXT_CLASS: &str = "hover-text";

    fn new(at: ViewportPoint, text: &str) -> Self {
        let element = Element::new("div");
        element.add_class(Self::HOVER_TEXT_CLASS);
        element.set_css("left", &format!("{}px", at.x));
        element.set_css("top", &format!("{}px", at.y));
        element.set_text(text);
        Self { element }
    }
}

impl Drop for HoverText {
    fn drop(&mut self) {
        self.element.remove();
    }
}

pub struct HoverTextManager {
    element: Element,
    text: Vec<HoverText>,
}

impl HoverTextManager {
    pub fn new() -> Self {
        Self {
            element: Element::by_id(HOVER_ROOT_ID).unwrap_or_default(),
            text: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.text.clear();
    }

    pub fn render(&mut self, at: ViewportPoint, text: &str) {
        let text = HoverText::new(at, text);
        self.element.append_child(&text.element);
        self.text.push(text);
    }
}
