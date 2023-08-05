const Pages = {
    Game: game_key => `/game/${game_key}`,
    GameOver: '/game_over',
    NewScene: project_key => `/project/${project_key}/scene/new`
};

const Api = {
    NewGame: '/api/game/new',
    SceneDetails: '/api/scene/details',
    TestClient: (game_key, client_key) => `/api/game/${game_key}/${client_key}`
};

const Icons = {
    check_circle: `{{ bootstrap_icon(check-circle) }}`,
    exclamation_triangle: `{{ bootstrap_icon(exclamation-triangle) }}`,
    pencil_square: `{{ bootstrap_icon(pencil-square) }}`,
    lock: `{{ bootstrap_icon(lock) }}`,
    unlock: `{{ bootstrap_icon(unlock) }}`,
    eye: `{{ bootstrap_icon(eye) }}`,
    eye_slash: `{{ bootstrap_icon(eye-slash) }}`
};

function get_icon(name) {
    return Icons[name.replace("-", "_")];
}

function element_or_string(element) {
    if (typeof element === "string") {
        return document.getElementById(element);
    }

    return element;
}

function form_to_json(form) {
    form = element_or_string(form);

    let form_prefix = form.id.replace("_form", "") + "_";

    let data = {};
    let i = 0;
    while (true) {
        let element = form.elements[i++];
        if (element === undefined) {
            break;
        }

        if (element.tagName === "INPUT") {
            let value = element.value;
            if (element.type == "number") {
                value = parseFloat(value);
            }

            data[element.id.replace(form_prefix, "")] = value;
        }
    }

    return data;
}

function field_error(form, field_name, message) {
    form = element_or_string(form);

    let input = form.querySelector("#" + field_name);
    input.setCustomValidity(message);

    let feedback = form.querySelector(`[data-feedback-for="${field_name}"]`);
    let feedback_text;
    if (feedback) {
        feedback_text = feedback.innerText;
        feedback.innerText = message;
    }

    const listener = () => {
        input.setCustomValidity("");

        if (feedback) {
            feedback.innerText = feedback_text;
        }

        input.removeEventListener("input", listener);
    };

    input.addEventListener("input", listener);
}

function form_error(form, message) {
    element_or_string(form)
        .querySelector("[data-role='error_message']")
        .innerText = message;
}

function post_form_json(form, callback = null) {
    form = element_or_string(form);

    let req = new XMLHttpRequest();

    req.onerror = () => {
        if (callback) {
            callback();
        }

        form_error(form, "Network error. Please try again later.");
    }

    req.onload = () => {
        if (callback) {
            callback(req.response);
        }

        if (!req.response) {
            form_error(form, "Network error. Please try again later.");
            return;
        }

        if (req.response.success) {
            let redirect = form.getAttribute("data-redirect");
            if (redirect) {
                window.location = redirect;
            }
        }
        else if (req.response.problem_field) {
            field_error(form, req.response.problem_field, req.response.message);
        }
        else {
            form_error(form, req.response.message);
        }
    }

    req.responseType = "json";
    req.open("POST", form.action);
    req.setRequestHeader("Content-Type", "application/json;charset=UTF-8");
    req.send(JSON.stringify(form_to_json(form)));
}

function submit_form(form) {
    form = element_or_string(form);

    if (form.classList.contains("needs-validation")) {
        form.classList.add("was-validated");
        if (!form.checkValidity()) {
            return;
        }
    }

    post_form_json(form);
}

const LoadingIconStates = {
    Idle: "loading-idle",
    Loading: "loading-loading",
    Success: "loading-success",
    Error: "loading-error"
};

function update_loading_icon(icon_id, state) {
    const loading_icon = document.getElementById(icon_id);
    if (!loading_icon) {
        return;
    }

    Object.values(LoadingIconStates).forEach(cls => {
        loading_icon.classList.remove(cls);
    });

    loading_icon.classList.add(state);
}

function request_icon_handling(req, onload, onerror, icon_id) {
    if (icon_id) {
        update_loading_icon(icon_id, LoadingIconStates.Loading);
        req.onload = () => {
            update_loading_icon(
                icon_id,
                (
                    req?.response?.success
                        ? LoadingIconStates.Success
                        : LoadingIconStates.Error
                )
            );

            if (req?.response?.message) {
                document.getElementById(icon_id).title = req.response.message;
            }

            if (onload) {
                onload(req.response);
            }
        };
        req.onerror = () => {
            update_loading_icon(icon_id, LoadingIconStates.Error);
            if (onerror) {
                onerror();
            }
        };
    }
    else {
        if (onload) {
            req.onload = () => onload(req.response);
        }
        req.onerror = onerror;
    }
}

function url_parts() {
    return location.pathname.split("/").filter(p => p.length);
}

function get(path, onload = null, onerror = null, icon_id = null) {
    let req = new XMLHttpRequest();
    request_icon_handling(req, onload, onerror, icon_id);
    req.responseType = "json";
    req.open("GET", path);
    req.send();
}

function post(path, data, onload = null, onerror = null, icon_id = null) {
    let req = new XMLHttpRequest();
    request_icon_handling(req, onload, onerror, icon_id);
    req.responseType = "json";
    req.open("POST", path);
    req.setRequestHeader("Content-Type", "application/json;charset=UTF-8");
    req.send(JSON.stringify(data));
}

function template_to_element(html) {
    return document
        .createRange()
        .createContextualFragment(html)
        .firstElementChild;
}
