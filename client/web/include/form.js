function form_to_json(form) {
    let form_prefix = form.id.replace("_form", "") + "_";

    let data = {};
    let i = 0;
    while (true) {
        let element = form.elements[i++];
        if (element === undefined) {
            break;
        }
        
        if (element.tagName === "INPUT") {
            data[element.id.replace(form_prefix, "")] = element.value;
        }
    }

    return JSON.stringify(data);
}

function field_error(form, field_name, message) {
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
    form.querySelector("[data-role='error_message']").innerText = message;

}

function post_form_json(form, callback = null) {
    let req = new XMLHttpRequest();

    req.onerror = () => {
        if (callback) {
            callback();
        }

        form_error(form, "Network error. Please try again later.");
    }

    req.onload = () => {
        if (callback) {
            callback(req.response ? req.response.success : false);
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
    req.send(form_to_json(form));
}

function submit_form(form) {
    if (form.classList.contains("needs-validation")) {
        form.classList.add("was-validated");
        if (!form.checkValidity()) {
            return;
        }
    }

    post_form_json(form);
}
