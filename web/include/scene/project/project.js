function save_project() {
    post(
        "/scene/save",
        {
            title: document.getElementById("scene_title").value,
            encoded: rust_funcs.export_scene()
        },
        resp => console.log(rust_funcs.load_scene(resp.scene))
    );
}
