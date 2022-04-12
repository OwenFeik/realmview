function save_project() {
    post(
        "/scene/save",
        rust_funcs.export_scene(),
        resp => rust_funcs.set_scene_ids(resp.project_id, resp.scene_id)
    );
}
