window.addEventListener("load", () => {
    set_up_copy_link_btn();

    const current_game_tab = document.getElementById("current_game_tab");
    current_game_tab.style.display = "none";

    const parts = url_parts();
    if (parts[0] != "game") {
        return;
    }

    const game_key = parts[1];
    if (!game_key) {
        return;
    }

    const join_game_link = document.getElementById("join_game_link"); 
    join_game_link.href = join_game_link.innerText = (
        window.location.protocol
        + "//"
        + window.location.host
        + "/game/"
        + game_key
    );

    // Set the active tab in the game offcanvas to the current game info
    // tab.
    current_game_tab.style.display = "";
    current_game_tab.click();

    // Hide the project offcanvas.
    document.querySelector(
        "button[aria-controls='project_offcanvas']"
    ).style.display = "none";

    // Hide the launch game tab.
    document.getElementById("launch_game_tab").style.display = "none";

    // Set up the scene switcher
    const scene_change = document.getElementById("scene_menu_change_scene"); 
    scene_change.oninput = () => change_scene(scene_change.value); 
});

function set_up_copy_link_btn() {
    const join_game_link = document.getElementById("join_game_link");
    const btn = document.getElementById("copy_join_game_link_btn");

    btn.onclick = () => navigator.clipboard.writeText(join_game_link.href);
}

function error_fn(id) {
    let error = document.getElementById(id);
    error.classList.add("d-none");
    return message => {
        if (message) {
            error.classList.remove("d-none");
            error.innerText = message;
        }
        else {
            error.classList.add("d-none");
        }
    };
}

function new_game() {
    const error = error_fn("launch_game_error");
    post(
        "/api/game/new",
        { "scene_key": selected_scene() },
        resp => {
            if (resp?.success) {
                window.location = resp.url;
            }
            else if (resp?.message) {
                error(resp.message);
            }
            else {
                error("Server error.");
            }
        },
        () => error("Network error.")
    );
}
