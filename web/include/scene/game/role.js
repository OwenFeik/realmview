const ROLE_EDITOR = 2;

function update_interface(role) {
    if (role > ROLE_EDITOR) {
        document.body.classList.add("role_editor");
    }
    else {
        document.body.classList.remove("role_editor");
    }
}
