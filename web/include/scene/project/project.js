var export_closure = null;

// Note: this is exported to Rust (extern)
function set_export_closure(closure) {
    export_closure = closure;
}

function save_project() {
    if (!export_closure) {
        return;
    }

    post("/scene/save", data);
}
