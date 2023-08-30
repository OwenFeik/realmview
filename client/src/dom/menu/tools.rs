use scene::perms::Role;

use crate::{
    dom::{element::Element, icon::Icon, input::InputGroup},
    start::VpRef,
    viewport::Tool,
};

pub struct ToolsMenu {
    inputs: InputGroup,
}

impl ToolsMenu {
    const KEY: &'static str = "Tool";

    const EDITOR_TOOLS: &'static [Icon] = &[Icon::Cursor, Icon::Arrows, Icon::Brush, Icon::Fog];
    const PLAYER_TOOLS: &'static [Icon] = &[Icon::Cursor, Icon::Arrows, Icon::Brush];

    pub fn new(vp: VpRef, role: Role) -> Self {
        let mut inputs = InputGroup::new(vp);
        inputs.add_icon_radio_handler(
            Self::KEY,
            if role.editor() {
                Self::EDITOR_TOOLS
            } else {
                Self::PLAYER_TOOLS
            },
            move |vp, icon| {
                vp.set_tool(match icon {
                    Icon::Cursor => Tool::Select,
                    Icon::Arrows => Tool::Pan,
                    Icon::Brush => Tool::Draw,
                    Icon::Fog if role.editor() => Tool::Fog,
                    _ => Tool::Select,
                })
            },
        );

        inputs.root().add_classes(&["accordion-item", "p-2"]);

        Self { inputs }
    }

    pub fn update_tool(&self, tool: Tool) {
        self.inputs.set_selected_icon_radio(
            Self::KEY,
            match tool {
                Tool::Draw => Icon::Brush,
                Tool::Fog => Icon::Fog,
                Tool::Pan => Icon::Arrows,
                Tool::Select => Icon::Cursor,
            },
        );
    }

    pub fn root(&self) -> &Element {
        self.inputs.root()
    }
}
