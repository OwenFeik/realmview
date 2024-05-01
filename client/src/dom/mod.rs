pub mod element;
pub mod icon;
pub mod input;
pub mod menu;

fn set_visible(el: &str, visible: bool) {
    if let Some(element) = element::Element::by_id(el) {
        if visible {
            element.show();
        } else {
            element.hide();
        }
    }
}

pub fn update_interface(role: scene::perms::Role) {
    set_visible("end_game_btn", role == scene::perms::Role::Owner);
}
