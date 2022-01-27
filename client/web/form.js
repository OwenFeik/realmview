function form_to_json(form) {
    let data = {};
    let i = 0;
    while (true) {
        let element = form.elements[i++];
        if (element === undefined) {
            break;
        }
        
        if (element.tagName === "INPUT") {
            data[element.id] = element.value;
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

function post_form_json(form) {
    let req = new XMLHttpRequest();

    req.onerror = () => {
        form.querySelector("#error_message")
            .innerText = "Network error. Please try again later.";
    }

    req.onload = () => {
        if (req.response.success) {
            window.location = form.getAttribute("data-redirect");
        }
        else if (req.response.problem_field) {
            field_error(form, req.response.problem_field, req.response.message);
        }
        else {
            form.querySelector("#error_message")
                .innerText = req.response.message;
        }
    }

    req.responseType = "json";
    req.open("POST", form.action);
    req.setRequestHeader("Content-Type", "application/json;charset=UTF-8");
    req.send(form_to_json(form));
}

function submit_form(form, route) {
    if (form.classList.contains("needs-validation")) {
        form.classList.add("was-validated");
        if (!form.checkValidity()) {
            return;
        }
    }

    post_form_json(form, route);
}
