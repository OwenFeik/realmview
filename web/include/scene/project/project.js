function save_project() {
    post(
        "/scene/save",
        rust_funcs.export_scene(),
        resp => console.log(rust_funcs.load_scene(resp.scene))
    );
}
