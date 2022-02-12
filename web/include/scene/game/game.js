function new_game() {
    let req = new XMLHttpRequest();

    req.onload = () => {
        if (req.response?.success) {
            window.location = req.response.url;
        }
    };

    req.responseType = "json";
    req.open("POST", "/game/new");
    req.send();
}
