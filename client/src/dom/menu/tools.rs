use crate::{
    dom::{element::Element, icon::Icon, input::InputGroup},
    start::VpRef,
    viewport::Tool,
};

pub struct ToolsMenu {
    inputs: InputGroup,
}

impl ToolsMenu {
    const KEY: &str = "Tool";

    pub fn new(vp: VpRef) -> Self {
        let mut inputs = InputGroup::new(vp);
        inputs.add_icon_radio_handler(
            Self::KEY,
            &[Icon::Cursor, Icon::Arrows, Icon::Brush, Icon::Fog],
            |vp, icon| {
                vp.set_tool(match icon {
                    Icon::Cursor => Tool::Select,
                    Icon::Arrows => Tool::Pan,
                    Icon::Brush => Tool::Draw,
                    Icon::Fog => Tool::Fog,
                    _ => Tool::Select,
                })
            },
        );

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
