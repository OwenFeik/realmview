function layers_list_entry(label) {
    // uses label
    return template_to_element(`{{ scene/layers/layer_list_item.html }}`);
}

function load_layers() {
    const list = document.getElementById("layers_list");
    
    while (list.children.length) {
        list.children[0].remove();
    }

    try {
        RustFuncs.scene_layers()
        .sort((a, b) => a.z - b.z)
        .forEach(layer => {
            list.appendChild(layers_list_entry(layer.title))
        });
    }
    catch {
        // Func probably not available yet.
    }
}
