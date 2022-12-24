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

    // if URL is /game/GAME_KEY, attempt to join game GAME_KEY
    if (parts.length === 2) {
        document.getElementById("game_key").value = game_key;
        join_game();
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

    // Hide the project offcanvas
    document.querySelector(
        "button[aria-controls='project_offcanvas']"
    ).style.display = "none";

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

function join_game() {
    let form = document.getElementById("join_game_form");
    form.classList.add("was-validated");
    if (!form.checkValidity()) {
        return;
    }

    let key_input = document.getElementById("game_key")
    let key = key_input.value.toUpperCase();

    const feedback_text = document.querySelector(
        "#game_key + .invalid-feedback"
    );
    const default_feedback = feedback_text.innerText;

    const remove_invalid = () => {
        form.classList.remove("was-validated");
        key_input.setCustomValidity("");
        key_input.removeEventListener("input", remove_invalid);
        feedback_text.innerText = default_feedback;
    };

    if (!/^[0-9A-F]{{{ constant(GAME_KEY_LENGTH) }}}$/.test(key)) {
        key_input.setCustomValidity("Invalid game key.");
        key_input.addEventListener("input", remove_invalid);
        return;
    }

    let req = new XMLHttpRequest();


    const set_invalid = msg => {
        key_input.setCustomValidity(msg);
        feedback_text.innerText = msg;
        key_input.addEventListener("input", remove_invalid);
        
        // When we fail to join a game, show the relevant offcanvas tab
        // with the feedback.
        document.querySelector(
            "button[data-bs-target='#game_offcanvas']"
        ).click();
        document.getElementById("join_game_tab").click();
    };

    req.onerror = () => set_invalid("Network error.");
    req.onload = () => {
        if (req.response) {
            if (req.response.success) {
                window.location = req.response.url;
            }
            else {
                set_invalid(req.response.message);
            }
        }
        else {
            set_invalid("Server error.");
        }
    };

    req.responseType = "json";
    req.open("POST", "/api/game/" + key);
    req.send();
}
