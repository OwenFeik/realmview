window.addEventListener(
    "load", () => call_when_ready("scene_layers", load_layers)
);

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

function load_layers() {
    const list = document.getElementById("layers_list");
    
    let selected = selected_layer();
    let name = selected_layer_name();

    while (list.children.length) {
        list.children[0].remove();
    }

    try {
        let selected_one = false;
        let layers = RustFuncs.scene_layers().sort((a, b) => b.z - a.z);
        layers.forEach(layer => {
            let entry = layers_list_entry(layer);

            if (layer.id === selected) {
                entry
                    .querySelector("input[name='layer_radio']")
                    .checked = true;
                selected_one = true;
            }
            list.appendChild(entry);
        });
        
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
            list.querySelector("input[name='layer_radio']").checked = true;
        }
    }
    catch {
        // Func probably not available yet.
    }
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
