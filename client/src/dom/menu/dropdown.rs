use std::{rc::Rc, sync::Mutex};

use serde::Serialize;

use super::LayerInfo;
use crate::{bridge::console_log, dom::element::Element, viewport::ViewportPoint};

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
pub enum CanvasDropdownEvent {
    Aura,
    Clone,
    Delete,
    Group,
    Layer(scene::Id),
    Ungroup,
}

struct DropdownItem<T: Serialize> {
    element: Element,
    value: T,
}

impl<T: Serialize> DropdownItem<T> {
    fn new(label: &str, value: T) -> Self {
        Self {
            element: item(label),
            value,
        }
    }

    fn from(value: T) -> Self {
        Self {
            element: item(
                &serde_json::ser::to_string(&value)
                    .expect("DropdownItem types should serialise properly."),
            ),
            value,
        }
    }
}

impl<T: Serialize> Drop for DropdownItem<T> {
    fn drop(&mut self) {
        self.element.remove();
    }
}

fn link(label: &str) -> Element {
    Element::anchor()
        .with_attr("href", "#")
        .with_text(label)
        .with_class("dropdown-item")
}

fn item(label: &str) -> Element {
    Element::item().with_child(&link(label))
}

fn submenu(label: &str) -> Element {
    Element::item().with_class("dropend").with_child(
        &link(label)
            .with_class("dropdown-toggle")
            .with_attr("data-bs-toggle", "dropdown"),
    )
}

type Output = Rc<Mutex<Option<CanvasDropdownEvent>>>;
type CanvasItem = DropdownItem<CanvasDropdownEvent>;

pub struct Dropdown {
    element: Element,
    event: Output,
    items: Vec<DropdownItem<CanvasDropdownEvent>>,
    layers_menu: Element,
    layers: Vec<DropdownItem<CanvasDropdownEvent>>,
}

impl Dropdown {
    pub fn new() -> Self {
        Self::sprite()
    }

    fn sprite() -> Self {
        let mut dropdown = Self {
            element: Self::element(),
            event: Rc::new(Mutex::new(None)),
            items: Vec::new(),
            layers_menu: Self::element(),
            layers: Vec::new(),
        };

        // Ensure we can place it on the canvas
        dropdown.element.set_css("position", "absolute");
        dropdown.element.add_to_page();

        for (label, event) in [
            ("Add aura", CanvasDropdownEvent::Aura),
            ("Clone", CanvasDropdownEvent::Clone),
            ("Delete", CanvasDropdownEvent::Delete),
            ("Group Selection", CanvasDropdownEvent::Group),
            ("Ungroup", CanvasDropdownEvent::Ungroup),
        ] {
            dropdown.add_item(dropdown.new_item(label, event));
        }

        // Move to layer dropdown
        let layer_item = submenu("Move to Layer");
        layer_item.append_child(&dropdown.layers_menu);
        dropdown.element.append_child(&layer_item);

        dropdown
    }

    fn element() -> Element {
        let element = Element::list();
        element.add_class("dropdown-menu");
        element
    }

    fn add_item(&mut self, item: CanvasItem) {
        self.element.append_child(&item.element);
        self.items.push(item);
    }

    fn new_item(&self, label: &str, event: CanvasDropdownEvent) -> CanvasItem {
        let mut item = DropdownItem::new(label, event);
        let dest = self.event.clone();
        item.element.set_onclick(Box::new(move |_| {
            if let Ok(mut lock) = dest.try_lock() {
                lock.replace(event);
            } else {
                console_log("Failed to lock dropdown mutex.");
            }
        }));

        item
    }

    pub fn event(&self) -> Option<CanvasDropdownEvent> {
        if let Ok(mut lock) = self.event.try_lock() {
            let event = lock.take();
            if event.is_some() {
                self.hide();
            }
            event
        } else {
            console_log("Failed to lock dropdown event.");
            None
        }
    }

    fn set_visible(&self, visible: bool) {
        const CSS_CLASS: &str = "show";

        if visible {
            self.element.add_class(CSS_CLASS);
        } else {
            self.element.remove_class(CSS_CLASS);
        }
    }

    pub fn show(&self, at: ViewportPoint) {
        self.element.set_pos(at);
        self.element.show();
        self.set_visible(true);
    }

    pub fn hide(&self) {
        self.set_visible(false);
    }

    pub fn update_layers(&mut self, layers: &[LayerInfo]) {
        self.layers.clear();
        for layer in layers {
            let item = self.new_item(&layer.title, CanvasDropdownEvent::Layer(layer.id));
            self.layers_menu.append_child(&item.element);
            self.layers.push(item);
        }
    }

    pub fn update_options(&self, hide: &[CanvasDropdownEvent]) {
        for item in &self.items {
            if hide.contains(&item.value) {
                item.element.hide();
            } else {
                item.element.show();
            }
        }
    }
}
