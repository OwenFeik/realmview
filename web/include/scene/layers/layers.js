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
    
    while (list.children.length) {
        list.children[0].remove();
    }

    try {
        RustFuncs.scene_layers()
        .sort((a, b) => b.z - a.z)
        .forEach(layer => {
            list.appendChild(layers_list_entry(layer))
        });
    }
    catch {
        // Func probably not available yet.
    }
}
