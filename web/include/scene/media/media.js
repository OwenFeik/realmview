var media_manager;
window.addEventListener("load", () => {
    view_media();

    media_manager = new MediaManager();
});

class MediaManager {
    constructor() {
        this.media = {};
    }

    load_media_with_id(texture_id, callback) {
        let image = this.media[texture_id];
        if (image) {
            if (image.complete) {
                callback(image);
            }
            else {
                image.addEventListener("load", () => callback(image));
            }
        }
        else {
            get(
                "/media/details/" + texture_id,
                resp => {
                    if (!resp.success) {
                        return;
                    }
    
                    let i = new Image();
                    i.src = resp.details.url;
                    i.setAttribute(
                        "{{ constant(DATA_ID_ATTR) }}",
                        resp.details.id
                    );
                    this.media[resp.details.id] = i;
                    i.addEventListener("load", () => callback(image));
                }
            );    
        }        
    }

    add_media_with_image(image) {
        let id = image.getAttribute("{{ constant(DATA_ID_ATTR) }}");
        if (id) {
            this.media[id] = image;
        }
    }
}

function preview_card(src, name) {
    // uses src and name
    return template_to_element(`{{ scene/media/preview_card.html }}`);
}

function media_card(media_item) {
    // Variables used in template
    let src = media_item.url;
    let id = media_item.id;
    let title = media_item.title;
    let card = template_to_element(`{{ scene/media/media_card.html }}`);
    
    let image = card.querySelector(".card-img-top");
    media_manager.add_media_with_image(image);

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
    req.open("GET", "/media/list");
    req.send();
}
