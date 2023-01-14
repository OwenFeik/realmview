# TODO

## Features
* Homepage should be different when logged in. Maybe just redirect to
    project page by default.
* Warn when saving over changes
* Keyboard shortcuts on hovering tools
* Alt-scroll to change stroke
* Select from selected layer first (possibly)
* Layer opacity
* Hide sprite menu when no sprite selected
* Use sprite menu to change texture
* Place media in directories
* Manage fog of war by player
* When the WebSocket is closed, try to reconnect or redirect to a
    "game ended" page
* Associate each client with a user. When a user connects a second time,
    terminate that user's other sessions
* Specify which users are allowed to join the game
* Feedback for when connecting to a game as a given CLIENT_KEY fails
* Players should only be able to interact with the foreground
* Each player gets a layer of their own automatically
* Display currently selected line caps
* Measurements, persistent measurements
* Hold a button to rotate a sprite
* Save perms, reload when the same scene is loaded
* Expose closure macro

## Bugs
* Undoing creation of a shape moves it to a 0x0 at cursor
* Updating sprite shape via menu is broken
* Tool selection gets broken when changing stroke
* When saving a scene, a new project is created

# Documentation

## Requirements

### Build Requirements

* Rust Nightly (`rustup default nightly`)
* `wasm-pack` (`cargo install wasm-pack`)
    * Note: `0.10.2`, `0.10.3` found to segfault when building. `0.9.0` known
        working. (`cargo install wasm-pack@0.9.0`)
* OpenSSL (`apt install libssl-dev` / `yum install openssl-devel`)
* Python 3.6

## Keyboard Shortcuts

| Key    | Tool          |
| ------ | ------------- |
| Space  | Pan           |
| Escape | `D` then `Q`  |
| `A`    | Select all    |
| `C`    | Copy          |
| `D`    | Deselect      |
| `E`    | Ellipse       |
| `F`    | Freehand draw |
| `L`    | Line draw     |
| `Q`    | Select        |
| `R`    | Rectangle     |
| `V`    | Paste         |
| `W`    | Edit fog      |
| `Y`    | Redo          |
| `Z`    | Undo          |
| `+`    | Zoom in       |
| `-`    | Zoom out      |

## Moving Sprites

* Use the select tool to drag sprites around. Click on a sprite and drag to
    move or draw a marquee to select multiple sprites.
* When dragging a sprite, if that sprite was initially aligned to the grid, it
    will be snapped to the grid on finishing the move. If it was initially
    unaligned, it will remain so. To change this behaviour, hold `Alt`. So If
    you want to unalign a sprite from the grid, hold `Alt` when releasing a
    drag. Likewise to snap an unaligned sprite to the grid. 
