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

    let req = new XMLHttpRequest();

    req.onerror = () => error("Network error.");
    req.onload = () => {
        if (req.response) {
            if (req.response.success) {
                window.location = req.response.url;
            }
            else {
                error(req.response.message);
            }
        }
        else {
            error("Server error.");
        }
    };

    req.responseType = "json";
    req.open("POST", "/game/new");
    req.send();
}

function join_game() {
    let form = document.getElementById("join_game_form");
    form.classList.add("was-validated");
    if (!form.checkValidity()) {
        return;
    }

    let key_input = document.getElementById("game_key")
    let key = key_input.value.toUpperCase();

    if (!/^[0-9A-F]{{{ constant(GAME_KEY_LENGTH) }}}$/.test(key)) {
        key_input.setCustomValidity("Invalid game key.");
        
        const remove_invalid = () => {
            form.classList.remove("was-validated");
            key_input.setCustomValidity("");
            key_input.removeEventListener("input", remove_invalid);
        };
        key_input.addEventListener("input", remove_invalid);

        return;
    }

    const error = error_fn("join_game_error");

    let req = new XMLHttpRequest();

    req.onerror = () => error("Network error.");
    req.onload = () => {
        if (req.response) {
            if (req.response.success) {
                window.location = req.response.url;
            }
            else {
                error(req.response.message);
            }
        }
        else {
            error("Server error.");
        }
    };

    req.responseType = "json";
    req.open("POST", "/game/" + key);
    req.send();
}
