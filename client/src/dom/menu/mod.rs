use ::scene::Id;

pub use self::dropdown::CanvasDropdownEvent;
pub use self::layers::LayerInfo;
use super::element::Element;
use crate::{interactor::details::SceneDetails, start::VpRef, viewport::ViewportPoint};

mod draw;
mod dropdown;
mod layers;
mod scene;
mod sprite;
mod tools;

const CAP_OPTIONS: &[(&str, &str)] = &[("Arrow", "arrow"), ("Round", "round"), ("None", "none")];

fn id(key: &str) -> String {
    format!("#{key}")
}

fn accordion_id(key: &str) -> String {
    format!("{}_menu", key.to_lowercase())
}

fn accordion_collapse_id(key: &str) -> String {
    format!("menu_{}_collapse", key.to_lowercase())
}

fn menu_element() -> Option<Element> {
    Element::by_id("canvas_menu")
}

fn add_accordion(el: &Element, key: &str, inputs: &Element) {
    inputs.add_class("p-2");

    let item = el
        .child("div")
        .with_attr("id", &accordion_id(key))
        .with_class("accordion-item");
    let heading = &format!("menu_{}_heading", key.to_lowercase());
    let collapse = &accordion_collapse_id(key);
    item.child("h2")
        .with_attr("id", heading)
        .with_class("accordion-header")
        .child("button")
        .with_classes(&["accordion-button", "shadow-none", "collapsed"])
        .with_attrs(&[
            ("type", "button"),
            ("data-bs-toggle", "collapse"),
            ("data-bs-target", &id(collapse)),
            ("aria-expanded", "false"),
            ("aria-controls", &id(collapse)),
        ])
        .with_text(key);
    item.child("div")
        .with_attr("id", collapse)
        .with_classes(&["accordion-collapse", "collapse"])
        .with_attr("aria-labelledby", &id(heading))
        .with_child(inputs);
}

fn toggle_accordion_if<F: Fn(&Element) -> bool>(key: &str, condition: F) {
    let collapse_id = accordion_collapse_id(key);
    if let Some(collapse) = Element::by_id(&collapse_id) {
        if condition(&collapse) {
            if let Some(button) =
                Element::by_selector(&format!("[data-bs-target='{}']", id(&collapse_id)))
            {
                button.click();
            }
        }
    }
}

fn show_accordion(key: &str) {
    toggle_accordion_if(key, |el| !el.has_class("show"))
}

fn hide_accordion(key: &str) {
    toggle_accordion_if(key, |el| el.has_class("show"))
}

fn set_accordion_visible(key: &str, visible: bool) {
    if visible {
        show_accordion(key);
    } else {
        hide_accordion(key);
    }
}

pub struct Menu {
    dropdown: dropdown::Dropdown,
    layers: layers::LayersMenu,
    scene: scene::SceneMenu,
    draw: draw::DrawMenu,
    sprite: sprite::SpriteMenu,
    tools: tools::ToolsMenu,
}

impl Menu {
    const DRAW: &str = "Draw";
    const SCENE: &str = "Scene";
    const SPRITE: &str = "Sprite";

    pub fn new(vp: VpRef) -> Self {
        let menu = Self {
            dropdown: dropdown::Dropdown::new(),
            layers: layers::LayersMenu::new(vp.clone()),
            scene: scene::SceneMenu::new(vp.clone()),
            draw: draw::DrawMenu::new(vp.clone()),
            sprite: sprite::SpriteMenu::new(vp.clone()),
            tools: tools::ToolsMenu::new(vp),
        };

        if let Some(el) = menu_element() {
            el.append_child(
                &menu
                    .tools
                    .root()
                    .clone()
                    .with_classes(&["accordion-item", "p-2"]),
            );
            add_accordion(&el, "Layers", menu.layers.root());
            add_accordion(&el, Self::SCENE, menu.scene.root());
            add_accordion(&el, Self::DRAW, menu.draw.root());
            add_accordion(&el, Self::SPRITE, menu.sprite.root());
        }

        menu
    }

    pub fn update_tool(&self, tool: crate::viewport::Tool) {
        use crate::viewport::Tool;

        set_accordion_visible(Self::DRAW, matches!(tool, Tool::Draw));
        set_accordion_visible(Self::SCENE, matches!(tool, Tool::Fog));

        self.tools.update_tool(tool);
    }

    pub fn get_draw_details(&self) -> crate::interactor::details::SpriteDetails {
        self.draw.details()
    }

    pub fn handle_stroke_change(&mut self, delta: f32) {
        self.draw.change_stroke(delta);
    }

    pub fn set_draw_tool(&mut self, draw_tool: crate::viewport::DrawTool) {
        self.draw.set_draw_tool(draw_tool);
    }

    pub fn show_dropdown(&self, at: ViewportPoint) {
        self.dropdown.show(at);
    }

    pub fn hide_dropdown(&self) {
        self.dropdown.hide();
    }

    pub fn dropdown_event(&mut self) -> Option<dropdown::CanvasDropdownEvent> {
        self.dropdown.event()
    }

    pub fn set_scene_details(&mut self, details: SceneDetails) {
        self.scene.set_details(details);
    }

    pub fn set_scene(&mut self, key: Option<String>) {
        self.scene.set_scene(key);
    }

    pub fn set_scene_list(&mut self, list: Vec<(String, String)>) {
        self.scene.set_scene_list(list);
    }

    pub fn set_fog_brush(&mut self, brush: f32) {
        self.scene.set_fog_brush(brush);
    }

    pub fn set_layer_info(&mut self, selected: Id, layers: &[LayerInfo]) {
        self.layers.update(selected, layers);
        self.dropdown.update_layers(layers);
    }

    pub fn set_sprite_info(&mut self, details: Option<crate::interactor::details::SpriteDetails>) {
        self.sprite.set_sprite_info(details);
    }

    pub fn update_selection(&mut self, has_selection: bool) {
        set_accordion_visible(Self::SPRITE, has_selection);
    }
}
