const Icons = {
    check_circle: `{{ bootstrap_icon(check-circle) }}`,
    exclamation_triangle: `{{ bootstrap_icon(exclamation-triangle) }}`
};

window.onload = () => {
    view_media();
    configure_media_details_modal();
};

function configure_media_details_modal() {
    document
        .getElementById("media_details_modal")
        .addEventListener("show.bs.modal", e => {
            let button = e.relatedTarget;
            
            let el = button.parentNode;
            while (!el.classList.contains("card")) {
                el = el.parentNode;
            }

            let image = el.querySelector(".card-img-top");
            document
                .getElementById("media_details_title")
                .value = image.getAttribute("data-title");
            document
                .getElementById("media_details_id")
                .value = image.getAttribute("data-id");

            form_error(document.getElementById("media_details_form"), "");
        });

    document.getElementById("media_details_save").onclick = () => {
        let loading = document.getElementById("media_details_loading");
        loading.classList.add("show");

        post_form_json(
            document.getElementById("media_details_form"),
            success => {
                loading.classList.remove("show");
                if (success) {
                    document
                        .getElementById("media_details_modal")
                        .querySelector(".btn-close")
                        .click();
                    view_media();
                }
            }
        );
    };
}

function template_to_element(html) {
    return document
        .createRange()
        .createContextualFragment(html)
        .firstElementChild;
}

function preview_card(src, name) {
    // uses src and name
    return template_to_element(`{{ preview_card.html }}`);
}

function media_card(media_item) {
    // Variables used in template
    let src = media_item.url;
    let id = media_item.id;
    let title = media_item.title;
    let card = template_to_element(`{{ media_card.html }}`);
    
    let image = card.querySelector(".card-img-top");

    // Buttons: [Add, Edit]
    let buttons = card.querySelectorAll("button");
    buttons[0].onclick = () => add_to_scene(image);

    return card;
}

function spinner_to_icon(spinner, icon, klasse) {
    spinner.classList.remove("spinner-border");
    spinner.innerHTML = icon;
    spinner.classList.add(klasse);
    spinner.firstChild.style.display = "block";
    spinner.firstChild.style.width = "1.25rem";
    spinner.firstChild.style.height = "1.25rem";
}

function set_card_error(card, message) {
    card.querySelector(".text-danger").innerText = message;
    let spinner = card.querySelector("div.spinner-border");
    spinner_to_icon(spinner, Icons.exclamation_triangle, "text-danger");
}

function set_card_success(card) {
    let spinner = card.querySelector("div.spinner-border");
    spinner_to_icon(spinner, Icons.check_circle, "text-success");
}

function upload_media() {
    const media_input = document.getElementById("media_upload");
    const media_preview = document.getElementById("media_upload_previews");

    media_preview.innerHTML = "";
    for (const file of media_input.files) {
        let card = preview_card(URL.createObjectURL(file), file.name);

        media_preview.appendChild(card);

        let data = new FormData();
        data.append("image", file);
        
        let req = new XMLHttpRequest();
    
        req.onerror = () => {
            set_card_error(card, "Network error.");
        };

        req.onload = () => {
            if (req.response.success) {
                set_card_success(card);
            }
            else {
                set_card_error(card, req.response.message);
            }
        };
    
        req.responseType = "json";
        req.open("POST", "/upload");
        req.send(data);
    }
}

function view_media() {
    let req = new XMLHttpRequest();
    let label = document.getElementById("media_view_error");

    let loading = document.getElementById("media_view_loading");
    loading.classList.add("show");
    
    let media_preview = document.getElementById("media_view_previews");
    media_preview.innerHTML = "";

    req.onerror = () => {
        label.classList.remove("d-none");
        label.innerText = "Network error.";
        loading.classList.remove("show");
    };

    req.onload = () => {
        if (!req.response) {
            label.classList.remove("d-none");
            label.innerText = "Network error.";
            loading.classList.remove("show");    
        }
        else if (req.response.success) {
            label.classList.add("d-none");
            req.response.items.forEach(item => {
                media_preview.appendChild(media_card(item));
            });
        }
        else {
            label.classList.remove("d-none");
            label.innerText = req.response.message;
        }

        loading.classList.remove("show");
    };
    
    req.responseType = "json";
    req.open("GET", "/media");
    req.send();
}
