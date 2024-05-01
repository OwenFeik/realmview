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
});

function end_game() {
    const game_key = url_parts()[1];
    const error = error_fn("end_game_error");
    post(
        Api.EndGame(game_key),
        null,
        resp => {
            if (resp?.success) {
                window.location.href = Pages.GameOver;
            } else if (resp?.message) {
                error(resp.message);
            } else {
                error("Server error.");
            }
        },
        () => error("Network error.")
    );
}

function set_up_copy_link_btn() {
    const join_game_link = document.getElementById("join_game_link");
    const btn = document.getElementById("copy_join_game_link_btn");

    if (window.navigator) {
        btn.onclick = () => window
            .navigator
            .clipboard
            .writeText(join_game_link.href);
    } else {
        btn.remove();
    }
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
        Api.NewGame,
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
