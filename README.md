# TODO

* Scene
    * Hide or show UI elements depending on user role.
    * Draw on map.
        * Shapes
            * Ellipse
            * Rectangle
            * Filled
            * Hollow
        * Freehand
            * Stroke width
    * Issue when dragging multiple sprites from different selections where
        different selections move different amounts.
* Layers
    * Select from selected layer first (possibly)
    * Layer opacity
* Sprites
    * Use sprite menu to change texture.
    * Change cursor when hovering a sprite or anchor.
* Media
    * Delete media items
    * Media library
        * Place media in directories
        * Multiselect media, apply action to all items
    * Pressing enter in modal breaks things
    * When duplicate media is rejected, database error is the response
* Projects
    * /project/PROJECT_KEY should show a page with the scenes in that project
        and project metadata
    * /project should show the same page but empty, with prompt to save the
        project and create a scene
* Game
    * Save the scene in the game
    * Associate each client with a user. When a user connects a second time,
        terminate that user's other sessions
    * Specify which users are allowed to join the game
    * Feedback for when connecting to a game as a given CLIENT_KEY fails
    * Permission levels
        * Editor should be able to configure permissions by layer, allowing or
            forbidding users to
            
            * Move sprites
            * Add sprites
    * Editors should be able to change the current scene to another in the
        project, should be able to save the project. 
* Callum requests
    * docs.google.com/document/d/1uKsAKS-huxNqc4kuHFot0McXTLlT3p83ojEalAcBtK0/
