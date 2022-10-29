class MediaItem {
    constructor(key, title, url) {
        this.key = key;
        this.title = title;
        this.url = url;

        this.card = template_to_element(
            `{{ media/card(IFDEF(ADD_BUTTON) {{ add_button=1 }}) }}`
        );

        this.image = this.card.querySelector("img");

        IFDEF(ADD_BUTTON) {{
            // Buttons: [Add, Edit]
            let buttons = this.card.querySelectorAll("button");
            buttons[0].onclick = () => add_to_scene(this.image);
        }}
    }
}

class MediaManager {
    constructor() {
        this.media = {};
    }

    add_item(resp_item) {
        let media_item = new MediaItem(
            resp_item.media_key, resp_item.title, resp_item.url
        );
        this.media[resp_item.media_key] = media_item; 
        return media_item;
    }

    remove_media(media_key) {
        delete this.media[media_key];
    }

    load_media_with_key(media_key, callback) {
        let media = this.media[media_key];
        if (media) {
            if (media.image.complete) {
                callback(media.image);
            }
            else {
                media.image.addEventListener(
                    "load", () => callback(media.image)
                );
            }
        }
        else {
            get(
                "/api/media/" + media_key,
                resp => {
                    if (!resp.success) {
                        return;
                    }

                    this.add_item(resp.details);
                    this.load_media_with_key(media_key, callback);
                }
            );    
        }        
    }
}

function preview_card(src, name) {
    // uses src and name
    return template_to_element(`{{ media/preview_card.html }}`);
}

function spinner_to_icon(spinner, icon, klasse) {
    spinner.classList.remove("spinner-border");
    spinner.innerHTML = icon;
    spinner.classList.add(klasse);
    spinner.firstChild.style.display = "inline-block";
}

function set_card_error(card, message) {
    card.querySelector(".text-danger").innerText = message;
    let spinner = card.querySelector(".spinner-border");
    spinner_to_icon(spinner, Icons.exclamation_triangle, "text-danger");
}

function set_card_success(card) {
    let spinner = card.querySelector(".spinner-border");
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
        req.open("POST", "/api/upload");
        req.send(data);
    }
}

function show_media(media_list) {
    let media_preview = document.getElementById("media_view_previews");
    media_preview.innerHTML = "";
    media_list.forEach(item => {
        media_preview.appendChild(item.card);
    });
}

function view_media() {
    let req = new XMLHttpRequest();
    let label = document.getElementById("media_view_error");

    let loading = document.getElementById("media_view_loading");
    loading.classList.add("show");
    
    show_media([]);

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
            show_media(
                req.response.items.map(item => media_manager.add_item(item))
            );
        }
        else {
            label.classList.remove("d-none");
            label.innerText = req.response.message;
        }

        loading.classList.remove("show");
    };
    
    req.responseType = "json";
    req.open("GET", "/api/media/list");
    req.send();
}

function delete_media_item(key) {
    modal_confirm(() => fetch(
        "/api/media/" + key,
        { method: "DELETE" }
    ).then(resp => {
        if (resp.ok) {
            document.getElementById("media_" + key)?.remove();
            media_manager.remove_media(key);
        }
    }));
}

function search_filter(query) {
    query = query.toLowerCase();
    let matching = Object
        .values(media_manager.media)
        .filter(item => item.title.toLowerCase().includes(query))
    show_media(matching);
}

var media_manager = new MediaManager();
window.addEventListener("load", view_media);
