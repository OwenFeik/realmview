function layers_list_entry(layer_info) {
    let el = document.createElement("li");
    el.classList.add("list-group-item");
    el.innerText = layer_info.title;
    return el;
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
            list.appendChild(layers_list_entry(layer))
        });
    }
    catch {
        // Func probably not available yet.
    }
}
