function layers_list_entry(layer) {
    let id = layer.id;
    let label = layer.title; // Used in below
    let el = template_to_element(`{{ scene/layers/layer_list_item.html }}`);

    if (!layer.visible) {
        let btn = el.querySelector(".bi-eye").parentNode; 
        btn.setAttribute("data-value", "0");
        btn.innerHTML = get_icon("eye-slash");
    }

    if (layer.locked) {
        let btn = el.querySelector(".bi-unlock").parentNode;
        btn.setAttribute("data-value", "0"); 
        btn.innerHTML = get_icon("lock");
    }

    return el;
}

function canvas_layers_list_entry(layer) {
    // Uses layer.id and layer.title
    return template_to_element(`{{ scene/layers/canvas_list_item.html }}`);
}

function update_canvas_layers_list(layers) {
    const list = document.getElementById("canvas_sprite_dropdown_layer_list");
    list.innerHTML = "";
    layers.forEach(layer => list.appendChild(canvas_layers_list_entry(layer)));
}

function update_layers_list(layers, selected) {
    const RADIO_SEL = "input[name='layer_radio']";

    const list = document.getElementById("layers_list");
    
    let name = selected_layer_name();

    while (list.children.length) {
        list.children[0].remove();
    }

    let selected_one = false;
    let z = Infinity;
    const divider = template_to_element('<hr class="mt-1 mb-0">');
    layers.forEach(layer => {
        let entry = layers_list_entry(layer);
        if (layer.id === selected) {
            entry
                .querySelector()
                .checked = true;
            selected_one = true;
        }

        // Insert a rule at the z height of the grid
        if (layer.z < 0 && z >= 0) {
            list.appendChild(divider);
        }
        z = layer.z;

        list.appendChild(entry);
    });

    if (z >= 0) {
        list.appendChild(divider);
    }
    
    if (!selected_one) {
        layers.forEach(layer => {
            if (layer.title === name) {
                list
                    .querySelector(radio_selector(layer.id))
                    .checked = true;
                selected_one = true;
            }
        })
    }

    if (!selected_one) {
        list.querySelector(RADIO_SEL).checked = true;
    }

    update_canvas_layers_list(layers);
}

function selected_layer() {
    return parseInt(
        document
            ?.getElementById("layers_list")
            ?.querySelector("input[name='layer_radio']:checked")
            ?.getAttribute("{{ constant(DATA_ID_ATTR) }}")
    ) || 0; 
}

function radio_selector(layer_id) {
    return "input[name='layer_radio'][data-id='ID']".replace('ID', layer_id);
}

function selected_layer_name() {
    return document
        .getElementById("layers_list")
        ?.querySelector(radio_selector(selected_layer()))
        ?.parentNode
        ?.parentNode
        ?.querySelector("input[type='text']")
        ?.value;
}
