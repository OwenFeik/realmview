# TODO

* Scene
    * Warn when saving over changes
* Layers
    * Select from selected layer first (possibly)
    * Layer opacity
* Sprites
    * Use sprite menu to change texture
    * Group sprites
* Media
    * Place media in directories
* Projects
* Game
    * Manage fog of war by player
    * Hide illegal dropdown options for players
    * When the WebSocket is closed, try to reconnect or redirect to a
        "game ended" page
    * Associate each client with a user. When a user connects a second time,
        terminate that user's other sessions
    * Specify which users are allowed to join the game
    * Feedback for when connecting to a game as a given CLIENT_KEY fails
    * Players should only be able to interact with the foreground
* Backend
    * When saving a scene, a new project is created
    * Save perms, reload when the same scene is loaded
    * Expose closure macro
* Build
* Callum requests
    * docs.google.com/document/d/1uKsAKS-huxNqc4kuHFot0McXTLlT3p83ojEalAcBtK0/

# Documentation

## Requirements

### Build Requirements

* Rust Nightly (`rustup default nightly`)
* `wasm-pack` (`cargo install wasm-pack`)
    * Note: `0.10.2`, `0.10.3` found to segfault when building. `0.9.0` known
        working.
* OpenSSL (`apt install libssl-dev` / `yum install openssl-devel`)
* Python 3.6

## Keyboard Shortcuts

* `Q`  Select

## Moving Sprites

* Use the select tool to drag sprites around. Click on a sprite and drag to
    move or draw a marquee to select multiple sprites.
* When dragging a sprite, if that sprite was initially aligned to the grid, it
    will be snapped to the grid on finishing the move. If it was initially
    unaligned, it will remain so. To change this behaviour, hold `Alt`. So If
    you want to unalign a sprite from the grid, hold `Alt` when releasing a
    drag. Likewise to snap an unaligned sprite to the grid. 
