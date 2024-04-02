use std::collections::HashMap;
use std::rc::Rc;

use js_sys::Float32Array;
use scene::{Colour, PointVector};
use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::{
    HtmlImageElement, WebGlBuffer, WebGlProgram, WebGlShader, WebGlTexture, WebGlUniformLocation,
};

use super::ViewInfo;
use crate::bridge::{log, Gl};
use crate::scene::{Point, Rect, Shape};

// 0 is the default and what is used here
const GL_TEXTURE_DETAIL_LEVEL: i32 = 0;

// Required to be 0 for textures
const GL_TEXTURE_BORDER_WIDTH: i32 = 0;

struct Texture {
    pub width: u32,
    pub height: u32,
    pub texture: WebGlTexture,
}

impl Texture {
    fn new(gl: &Gl) -> anyhow::Result<Texture> {
        Ok(Texture {
            width: 0,
            height: 0,
            texture: Texture::create_gl_texture(gl)?,
        })
    }

    fn gen_mipmap(&self, gl: &Gl) {
        gl.bind_texture(Gl::TEXTURE_2D, Some(&self.texture));
        gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_WRAP_S, Gl::CLAMP_TO_EDGE as i32);
        gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_WRAP_T, Gl::CLAMP_TO_EDGE as i32);
        gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_MIN_FILTER, Gl::LINEAR as i32);
    }

    fn create_gl_texture(gl: &Gl) -> anyhow::Result<WebGlTexture> {
        match gl.create_texture() {
            Some(t) => Ok(t),
            None => Err(anyhow::anyhow!("Unable to create texture.")),
        }
    }

    fn load_u8_array(
        &mut self,
        gl: &Gl,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> anyhow::Result<()> {
        gl.bind_texture(Gl::TEXTURE_2D, Some(&self.texture));

        if gl
            .tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
                Gl::TEXTURE_2D,
                GL_TEXTURE_DETAIL_LEVEL,
                Gl::RGBA as i32,
                width as i32,
                height as i32,
                GL_TEXTURE_BORDER_WIDTH,
                Gl::RGBA,
                Gl::UNSIGNED_BYTE, // u8
                Some(data),
            )
            .is_err()
        {
            return Err(anyhow::anyhow!("Unable to load array as texture."));
        }

        self.gen_mipmap(gl);

        self.width = width;
        self.height = height;

        Ok(())
    }

    fn from_u8_array(gl: &Gl, width: u32, height: u32, data: &[u8]) -> anyhow::Result<Texture> {
        let mut texture = Texture::new(gl)?;
        texture.load_u8_array(gl, width, height, data)?;
        Ok(texture)
    }

    fn from_html_image(gl: &Gl, image: &HtmlImageElement) -> anyhow::Result<Texture> {
        let mut texture = Texture::new(gl)?;
        texture.load_html_image(gl, image)?;

        Ok(texture)
    }

    fn load_html_image(&mut self, gl: &Gl, image: &HtmlImageElement) -> anyhow::Result<()> {
        Texture::load_html_image_gl_texture(gl, image, &self.texture)?;
        self.width = image.natural_width();
        self.height = image.natural_height();
        self.gen_mipmap(gl);

        Ok(())
    }

    fn load_html_image_gl_texture(
        gl: &Gl,
        image: &HtmlImageElement,
        texture: &WebGlTexture,
    ) -> anyhow::Result<()> {
        gl.bind_texture(Gl::TEXTURE_2D, Some(texture));

        if gl
            .tex_image_2d_with_u32_and_u32_and_html_image_element(
                Gl::TEXTURE_2D,
                GL_TEXTURE_DETAIL_LEVEL,
                Gl::RGBA as i32,
                Gl::RGBA,
                Gl::UNSIGNED_BYTE,
                image,
            )
            .is_err()
        {
            return Err(anyhow::anyhow!("Failed to create WebGL image."));
        }

        Ok(())
    }

    fn from_url(
        gl: &Gl,
        url: &str,
        callback: Box<dyn Fn(anyhow::Result<Texture>)>,
    ) -> anyhow::Result<()> {
        // Create HTML image to load image from url
        let image = match HtmlImageElement::new() {
            Ok(i) => Rc::new(i),
            Err(_) => return Err(anyhow::anyhow!("Unable to create image element.")),
        };
        image.set_cross_origin(Some("")); // ?

        // Set callback to update texture once image is loaded
        {
            let gl = Rc::new(gl.clone());
            let image_ref = image.clone();
            let closure = Closure::wrap(Box::new(move || {
                callback(Texture::from_html_image(&gl, &image_ref));
            }) as Box<dyn FnMut()>);
            image.set_onload(Some(closure.as_ref().unchecked_ref()));
            closure.forget();
        }

        // Load image
        image.set_src(url);

        Ok(())
    }
}

pub struct TextureManager {
    gl: Rc<Gl>,
    textures: HashMap<scene::Id, Texture>,
    loading: Vec<scene::Id>,
}

impl TextureManager {
    pub fn new(gl: Rc<Gl>) -> anyhow::Result<TextureManager> {
        let missing_texture = Texture::from_u8_array(&gl, 1, 1, &[0, 0, 255, 255])?;
        let mut tm = TextureManager {
            gl,
            textures: HashMap::new(),
            loading: Vec::new(),
        };
        tm.add_texture(0, missing_texture);
        Ok(tm)
    }

    pub fn load_image(&mut self, image: &HtmlImageElement) -> scene::Id {
        let id = match image.get_attribute("data-media_key") {
            Some(s) => super::parse_media_key(&s),
            None => 0,
        };

        if id != 0 {
            match Texture::from_html_image(&self.gl, image) {
                Ok(t) => self.textures.insert(id, t),
                Err(_) => return 0,
            };
        } else {
            log("Texture manager was asked to load texture without ID.");
        }

        id
    }

    // NB will overwrite existing texture of this id
    fn add_texture(&mut self, id: scene::Id, texture: Texture) {
        self.textures.insert(id, texture);
        self.loading.retain(|&i| i != id);
    }

    // Returns the requested texture, queueing it to load if necessary.
    // (yay side effects!)
    pub fn get_texture(&mut self, id: scene::Id) -> &WebGlTexture {
        if let Some(tex) = self.textures.get(&id) {
            &tex.texture
        } else {
            if !self.loading.contains(&id) {
                self.loading.push(id);
                crate::bridge::load_texture(format!("{id:016X}"));
            }

            // This unwrap is safe because we always add a missing texture
            // texture as id 0 in the constructor.
            &self.textures.get(&0).unwrap().texture
        }
    }
}

struct Mesh {
    coords: Float32Array,
    position_buffer: WebGlBuffer,
    position_location: u32,
    matrix_location: WebGlUniformLocation,
    vertex_count: i32,
    scale: bool,
    translate: bool,
}

impl Mesh {
    // Requires that the program use "a_position" and "u_matrix"
    fn new(gl: &Gl, program: &WebGlProgram, points: &[f32]) -> anyhow::Result<Self> {
        let coords = Float32Array::new_with_length(points.len() as u32);
        coords.copy_from(points);

        let position_location = gl.get_attrib_location(program, "a_position") as u32;
        let position_buffer = create_buffer(gl, Some(&coords))?;

        let matrix_location = get_uniform_location(gl, program, "u_matrix")?;

        let vertex_count = (coords.length() / 2) as i32;

        Ok(Mesh {
            coords,
            position_buffer,
            position_location,
            matrix_location,
            vertex_count,
            scale: true,
            translate: true,
        })
    }

    fn set_transforms(&mut self, scale: bool, translate: bool) {
        self.scale = scale;
        self.translate = translate;
    }

    fn from_sprite_shape(gl: &Gl, program: &WebGlProgram, shape: Shape) -> anyhow::Result<Self> {
        Self::new(gl, program, &super::shapes::shape(shape))
    }

    // Should be called after using a program.
    fn draw(&self, gl: &Gl, vp: Rect, at: Rect) {
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&self.position_buffer));
        gl.enable_vertex_attrib_array(self.position_location);
        gl.vertex_attrib_pointer_with_i32(self.position_location, 2, Gl::FLOAT, false, 0, 0);

        let mut m = m4_orthographic(0.0, vp.w, vp.h, 0.0, -1.0, 1.0);

        if self.translate {
            m4_translate(&mut m, at.x - vp.x, at.y - vp.y, 0.0);
        }

        if self.scale {
            m4_scale(&mut m, at.w, at.h, 1.0);
        }

        gl.uniform_matrix4fv_with_f32_array(Some(&self.matrix_location), false, &m);
        gl.draw_arrays(Gl::TRIANGLES, 0, self.vertex_count);
    }

    fn draw_unscaled(&self, gl: &Gl, at: Point) {
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&self.position_buffer));
        gl.enable_vertex_attrib_array(self.position_location);
        gl.vertex_attrib_pointer_with_i32(self.position_location, 2, Gl::FLOAT, false, 0, 0);

        let mut m = m4_orthographic(0.0, 1.0, 1.0, 0.0, -1.0, 1.0);

        m4_translate(&mut m, at.x, at.y, 0.0);

        gl.uniform_matrix4fv_with_f32_array(Some(&self.matrix_location), false, &m);
        gl.draw_arrays(Gl::TRIANGLES, 0, self.vertex_count);
    }
}

struct Shapes {
    ellipse: Mesh,
    hexagon: Mesh,
    rectangle: Mesh,
    triangle: Mesh,
}

impl Shapes {
    fn new(gl: &Gl, program: &WebGlProgram) -> anyhow::Result<Self> {
        Ok(Shapes {
            ellipse: Mesh::from_sprite_shape(gl, program, Shape::Ellipse)?,
            hexagon: Mesh::from_sprite_shape(gl, program, Shape::Hexagon)?,
            rectangle: Mesh::from_sprite_shape(gl, program, Shape::Rectangle)?,
            triangle: Mesh::from_sprite_shape(gl, program, Shape::Triangle)?,
        })
    }

    fn shape(&self, shape: Shape) -> &Mesh {
        match shape {
            Shape::Ellipse => &self.ellipse,
            Shape::Hexagon => &self.hexagon,
            Shape::Rectangle => &self.rectangle,
            Shape::Triangle => &self.triangle,
        }
    }
}

pub struct SolidRenderer {
    gl: Rc<Gl>,
    program: WebGlProgram,
    colour_location: WebGlUniformLocation,
    shapes: Shapes,
}

impl SolidRenderer {
    pub fn new(gl: Rc<Gl>) -> anyhow::Result<Self> {
        let program = create_program(
            &gl,
            include_str!("shaders/solid.vert"),
            include_str!("shaders/single.frag"),
        )?;

        let colour_location = get_uniform_location(&gl, &program, "u_colour")?;
        let shapes = Shapes::new(&gl, &program)?;

        Ok(SolidRenderer {
            gl,
            program,
            colour_location,
            shapes,
        })
    }

    fn prepare_draw(&self, colour: Colour) {
        self.gl.use_program(Some(&self.program));
        self.gl
            .uniform4fv_with_f32_array(Some(&self.colour_location), colour.arr());
    }

    fn draw(&self, shape: &Mesh, colour: Colour, viewport: Rect, position: Rect) {
        self.prepare_draw(colour);
        shape.draw(&self.gl, viewport, position);
    }

    fn draw_unscaled(&self, shape: &Mesh, colour: Colour, vp: Rect, position: Rect) {
        self.prepare_draw(colour);
        shape.draw_unscaled(
            &self.gl,
            (position.top_left() - vp.top_left()) / Point::new(vp.w, vp.h),
        );
    }

    pub fn draw_shape(&self, shape: Shape, colour: Colour, viewport: Rect, position: Rect) {
        self.draw(self.shapes.shape(shape), colour, viewport, position);
    }
}

pub struct DrawingRenderer {
    gl: Rc<Gl>,
    grid_size: f32,
    drawings: HashMap<scene::Id, (u128, Mesh)>,
    renderer: SolidRenderer,
}

impl DrawingRenderer {
    pub fn new(gl: Rc<Gl>) -> anyhow::Result<Self> {
        Ok(Self {
            gl: gl.clone(),
            grid_size: 0.0,
            drawings: HashMap::new(),
            renderer: SolidRenderer::new(gl)?,
        })
    }

    fn create_key(
        drawing: &scene::Drawing,
        stroke: f32,
        cap_start: scene::Cap,
        cap_end: scene::Cap,
    ) -> u128 {
        // Key format is a u128 with the following structure:
        //
        // 32 bits for the rect width
        // 32 bits for the rect height
        // 32 bits for the stroke width
        // 27 bits counting the number of points in the drawing
        // 2 bits for the starting cap
        // 2 bits for the ending cap
        // 1 bit for whether the drawing is finished
        //
        // Like so:
        // 000000000000000000000000000WIDTH00000000000000000000000000HEIGHT
        // 00000000000000000000000000STROKE000000000000000N_POINTSSTRT0ENDF
        //
        // Is this grotesquely overcomplicated? Yes.
        let mut key = 0u128;

        let rect = drawing.rect();

        // First 100 bits are the literal bits of the three floats.
        let mut keyf32 = |v: f32| {
            key |= v.to_bits() as u128;
            key <<= 32;
        };
        keyf32(rect.w); // 32
        keyf32(rect.h); // 64
        keyf32(stroke); // 96

        // Last 28 bits. We've already shifted the first 100 across by 100.
        let mut low = 0u32;
        low |= drawing.n_points();
        low <<= 27; // Assume n_points is smaller than 27 bits.
        low |= cap_start as u32; // allow 2 bits
        low <<= 2;
        low |= cap_end as u32; // 2 bits
        key |= low as u128;
        key |= drawing.finished as u128; // final single bit

        key
    }

    fn add_drawing(
        &mut self,
        id: scene::Id,
        rect: scene::Rect,
        drawing: &scene::Drawing,
        stroke: f32,
        cap_start: scene::Cap,
        cap_end: scene::Cap,
    ) -> anyhow::Result<()> {
        // Before drawing is finished the sprite will always be 1*1. Only after
        // it is finished do we need to think about it being resized.
        let points_rect = drawing.rect();
        let (scale_x, scale_y) = if drawing.finished {
            (
                self.grid_size * rect.w / points_rect.w,
                self.grid_size * rect.h / points_rect.h,
            )
        } else {
            (self.grid_size, self.grid_size)
        };

        let points = match drawing.mode {
            scene::DrawingMode::Freehand => super::shapes::freehand(
                drawing.points().unwrap(),
                stroke,
                cap_start,
                cap_end,
                scale_x,
                scale_y,
            ),
            scene::DrawingMode::Line => {
                super::shapes::line(drawing.line(), stroke, cap_start, cap_end, self.grid_size)
            }
            scene::DrawingMode::Cone => super::shapes::cone(drawing.line(), self.grid_size),
        };

        let mut mesh = Mesh::new(&self.gl, &self.renderer.program, &points)?;
        mesh.set_transforms(false, true);
        self.drawings.insert(
            id,
            (Self::create_key(drawing, stroke, cap_start, cap_end), mesh),
        );
        Ok(())
    }

    fn get_drawing(
        &self,
        id: scene::Id,
        drawing: &scene::Drawing,
        stroke: f32,
        start: scene::Cap,
        end: scene::Cap,
    ) -> Option<&Mesh> {
        if let Some((key, mesh)) = self.drawings.get(&id) {
            if Self::create_key(drawing, stroke, start, end) == *key {
                return Some(mesh);
            }
        }
        None
    }

    fn update_grid_size(&mut self, grid_size: f32) {
        if self.grid_size != grid_size {
            self.grid_size = grid_size;
            self.drawings.clear();
        }
    }

    pub fn draw_drawing(
        &mut self,
        drawing: &scene::Drawing,
        stroke: f32,
        start: scene::Cap,
        end: scene::Cap,
        colour: scene::Colour,
        viewport: Rect,
        position: Rect,
        grid_size: f32,
    ) {
        self.update_grid_size(grid_size);
        let id = drawing.id;
        if let Some(mesh) = self.get_drawing(id, drawing, stroke, start, end) {
            self.renderer
                .draw(mesh, colour, viewport, position * grid_size);
        } else if self
            .add_drawing(id, position, drawing, stroke, start, end)
            .is_ok()
        {
            self.draw_drawing(
                drawing, stroke, start, end, colour, viewport, position, grid_size,
            );
        }
    }
}

pub struct HollowRenderer {
    gl: Rc<Gl>,
    grid_size: f32,
    meshes: HashMap<scene::Id, (u8, u32, f32, f32, f32, Mesh)>, // { id: (n, stroke, rect, mesh) }
    renderer: SolidRenderer,
}

impl HollowRenderer {
    pub fn new(gl: Rc<Gl>) -> anyhow::Result<Self> {
        Ok(Self {
            gl: gl.clone(),
            grid_size: 0.0,
            meshes: HashMap::new(),
            renderer: SolidRenderer::new(gl)?,
        })
    }

    fn add_shape(
        &mut self,
        id: scene::Id,
        shape: scene::Shape,
        stroke: f32,
        viewport: Rect,
        position: Rect,
    ) -> anyhow::Result<()> {
        let points = super::shapes::hollow_shape(
            shape,
            Rect {
                x: (stroke * self.grid_size) / viewport.w,
                y: (stroke * self.grid_size) / viewport.h,
                w: position.w / viewport.w,
                h: position.h / viewport.h,
            },
        );
        let mut mesh = Mesh::new(&self.gl, &self.renderer.program, &points)?;
        mesh.set_transforms(false, true);
        self.meshes
            .insert(id, (shape as u8, 1, stroke, position.w, position.h, mesh));
        Ok(())
    }

    fn get_mesh(
        &self,
        id: scene::Id,
        shape: scene::Shape,
        points: u32,
        stroke: f32,
        rect: Rect,
    ) -> Option<&Mesh> {
        if let Some((shp, n, s, w, h, mesh)) = self.meshes.get(&id) {
            // If n is different, drawing has changed, we don't have it
            if shape as u8 == *shp && points == *n && stroke == *s && rect.w == *w && rect.h == *h {
                return Some(mesh);
            }
        }
        None
    }

    fn update_grid_size(&mut self, grid_size: f32) {
        if self.grid_size != grid_size {
            self.grid_size = grid_size;
            self.meshes.clear();
        }
    }

    pub fn draw_shape(
        &mut self,
        id: scene::Id,
        shape: scene::Shape,
        colour: Colour,
        stroke: f32,
        viewport: Rect,
        position: Rect,
        grid_size: f32,
    ) {
        let pos = position.positive_dimensions();
        self.update_grid_size(grid_size);
        if let Some(shape) = self.get_mesh(id, shape, 1, stroke, pos) {
            self.renderer.draw_unscaled(shape, colour, viewport, pos);
        } else if self.add_shape(id, shape, stroke, viewport, pos).is_ok() {
            self.draw_shape(id, shape, colour, stroke, viewport, pos, grid_size);
        }
    }
}

struct TextureShapeRenderer {
    gl: Rc<Gl>,
    program: WebGlProgram,
    texcoord_buffer: WebGlBuffer,
    texcoord_location: u32,
    texture_location: WebGlUniformLocation,
    shape: Mesh,
}

impl TextureShapeRenderer {
    fn new(gl: Rc<Gl>, shape: Shape) -> anyhow::Result<Self> {
        let program = create_program(
            &gl,
            include_str!("shaders/solid.vert"),
            include_str!("shaders/image.frag"),
        )?;

        let shape = Mesh::from_sprite_shape(&gl, &program, shape)?;

        let texcoord_location = gl.get_attrib_location(&program, "a_texcoord") as u32;
        let texcoord_buffer = create_buffer(&gl, Some(&shape.coords))?;
        let texture_location = get_uniform_location(&gl, &program, "u_texture")?;

        Ok(TextureShapeRenderer {
            gl,
            program,
            texcoord_buffer,
            texcoord_location,
            texture_location,
            shape,
        })
    }

    fn draw_texture(&self, texture: &WebGlTexture, viewport: Rect, position: Rect) {
        let gl = &self.gl;

        gl.bind_texture(Gl::TEXTURE_2D, Some(texture));
        gl.use_program(Some(&self.program));
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&self.texcoord_buffer));
        gl.enable_vertex_attrib_array(self.texcoord_location);
        gl.vertex_attrib_pointer_with_i32(self.texcoord_location, 2, Gl::FLOAT, false, 0, 0);

        gl.uniform1i(Some(&self.texture_location), 0);
        self.shape.draw(gl, viewport, position);
    }
}

pub struct TextureRenderer {
    ellipse: TextureShapeRenderer,
    hexagon: TextureShapeRenderer,
    rectangle: TextureShapeRenderer,
    triangle: TextureShapeRenderer,
}

impl TextureRenderer {
    pub fn new(gl: Rc<Gl>) -> anyhow::Result<Self> {
        Ok(TextureRenderer {
            ellipse: TextureShapeRenderer::new(gl.clone(), Shape::Ellipse)?,
            hexagon: TextureShapeRenderer::new(gl.clone(), Shape::Hexagon)?,
            rectangle: TextureShapeRenderer::new(gl.clone(), Shape::Rectangle)?,
            triangle: TextureShapeRenderer::new(gl, Shape::Triangle)?,
        })
    }

    pub fn draw_texture(
        &self,
        shape: Shape,
        texture: &WebGlTexture,
        viewport: Rect,
        position: Rect,
    ) {
        match shape {
            Shape::Ellipse => self.ellipse.draw_texture(texture, viewport, position),
            Shape::Hexagon => self.hexagon.draw_texture(texture, viewport, position),
            Shape::Rectangle => self.rectangle.draw_texture(texture, viewport, position),
            Shape::Triangle => self.triangle.draw_texture(texture, viewport, position),
        }
    }
}

pub struct LineRenderer {
    gl: Rc<Gl>,
    program: WebGlProgram,
    position_location: u32,
    position_buffer: WebGlBuffer,
    colour_location: WebGlUniformLocation,
    point_count: i32,
}

impl LineRenderer {
    pub fn new(gl: Rc<Gl>) -> anyhow::Result<LineRenderer> {
        let program = create_program(
            &gl,
            include_str!("shaders/pos.vert"),
            include_str!("shaders/single.frag"),
        )?;
        let position_location = gl.get_attrib_location(&program, "a_position") as u32;
        let position_buffer = create_buffer(&gl, None)?;
        let colour_location = get_uniform_location(&gl, &program, "u_colour")?;

        Ok(LineRenderer {
            gl,
            program,
            position_location,
            position_buffer,
            colour_location,
            point_count: 0,
        })
    }

    pub fn scale_and_load_points(&mut self, points: &mut [f32], vp_w: f32, vp_h: f32) {
        for (i, v) in points.iter_mut().enumerate() {
            // Point vectors are of form [x1, y1, x2, y2 ... xn, yn] so even indices are xs.
            if i % 2 == 0 {
                *v = to_unit(*v, vp_w);
            } else {
                *v = -to_unit(*v, vp_h);
            }
        }

        self.load_points(points);
    }

    pub fn load_points(&mut self, points: &[f32]) {
        let positions = Float32Array::from(points);

        self.gl
            .bind_buffer(Gl::ARRAY_BUFFER, Some(&self.position_buffer));
        self.gl.buffer_data_with_opt_array_buffer(
            Gl::ARRAY_BUFFER,
            Some(&positions.buffer()),
            Gl::STATIC_DRAW,
        );
        self.point_count = (points.len() / 2) as i32;
    }

    fn prepare_render(&self, colour: Option<Colour>) {
        let gl = &self.gl;

        gl.use_program(Some(&self.program));
        gl.enable_vertex_attrib_array(self.position_location);
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&self.position_buffer));
        gl.vertex_attrib_pointer_with_i32(self.position_location, 2, Gl::FLOAT, false, 0, 0);
        gl.uniform4fv_with_f32_array(
            Some(&self.colour_location),
            &colour.map(Colour::raw).unwrap_or([0.5, 0.5, 0.5, 0.75]),
        );
    }

    fn render_lines(&self, colour: Option<Colour>) {
        self.prepare_render(colour);
        self.gl.draw_arrays(Gl::LINES, 0, self.point_count);
    }

    pub fn render_line_loop(&self, colour: Option<Colour>) {
        self.prepare_render(colour);
        self.gl.draw_arrays(Gl::LINE_LOOP, 0, self.point_count);
    }

    pub fn render_solid(&self, colour: Option<Colour>) {
        self.prepare_render(colour);
        self.gl.draw_arrays(Gl::TRIANGLES, 0, self.point_count);
    }
}

pub struct GridRenderer {
    line_renderer: LineRenderer,
    current_vp: Option<Rect>,
    current_grid_dims: Option<(u32, u32)>,
    current_grid_size: Option<f32>,
    current_line_count: Option<i32>,
}

impl GridRenderer {
    pub fn new(gl: Rc<Gl>) -> anyhow::Result<GridRenderer> {
        Ok(GridRenderer {
            line_renderer: LineRenderer::new(gl)?,
            current_vp: None,
            current_grid_dims: None,
            current_grid_size: None,
            current_line_count: None,
        })
    }

    pub fn create_grid(&mut self, vp: Rect, dims: (u32, u32), grid_size: f32) {
        let mut verticals = Vec::new();
        let mut horizontals = Vec::new();

        let d = grid_size;
        let dx = vp.x % grid_size;
        let dy = vp.y % grid_size;

        let w = vp.w;
        let h = vp.h;

        let (sw, sh) = dims;
        let sw = sw as f32;
        let sh = sh as f32;

        // Horizontal and vertical line start and endpoints, to ensure that we
        // render only the tiles that are part of the scene as part of the
        // grid.
        let fx = if vp.x < 0.0 {
            to_unit(vp.x.abs() / d, w / d)
        } else {
            -1.0
        };
        let tx = if (vp.x + w) / d > sw {
            to_unit(sw - vp.x / d, w / d).clamp(-1.0, 1.0)
        } else {
            1.0
        };
        let fy = if vp.y < 0.0 {
            -to_unit(vp.y.abs() / d, h / d)
        } else {
            1.0
        };
        let ty = if (vp.y + h) / d > sh {
            -to_unit(sh - vp.y / d, h / d).clamp(-1.0, 1.0)
        } else {
            -1.0
        };

        let mut i = 0.0;
        while i <= vp.w.max(vp.h) / d {
            let sx = i + (vp.x - dx) / d;
            let mut x = d * i - dx;
            if x <= w && sx >= 0.0 && sx <= sw {
                x = to_unit(x, w);

                verticals.push(x);
                verticals.push(fy);
                verticals.push(x);
                verticals.push(ty);
            }

            let sy = i + (vp.y - dy) / d;
            let mut y = d * i - dy;
            if y <= h && sy >= 0.0 && sy <= sh {
                // I negate the expression here but not for the x because the OpenGL coordinate system naturally matches
                // the browser coordinate system in the x direction, but opposes it in the y direction. By negating the
                // two coordinate systems are aligned, which makes things a little easier to work with.
                y = -to_unit(y, h);

                horizontals.push(fx);
                horizontals.push(y);
                horizontals.push(tx);
                horizontals.push(y);
            }

            i += 1.0;
        }

        verticals.append(&mut horizontals);
        self.line_renderer.load_points(&verticals);
        self.current_vp = Some(vp);
        self.current_grid_dims = Some(dims);
        self.current_grid_size = Some(grid_size);
        self.current_line_count = Some(verticals.len() as i32 / 2);
    }

    pub fn render_grid(&mut self, vp: ViewInfo, dimensions: (u32, u32)) {
        if self.current_vp.is_none()
            || self.current_vp.unwrap() != vp.viewport
            || self.current_grid_dims.is_none()
            || self.current_grid_dims.unwrap() != dimensions
            || self.current_grid_size != Some(vp.grid_size)
        {
            self.create_grid(vp.viewport, dimensions, vp.grid_size);
        }

        self.line_renderer.render_lines(None);
    }
}

pub struct FogRenderer {
    solid_renderer: SolidRenderer,
    shape: Option<Mesh>,
    current_vp: Option<Rect>,
    current_dimensions: Option<(u32, u32)>,
    current_grid_size: Option<f32>,
    current_n_revelead: Option<u32>,
}

impl FogRenderer {
    pub fn new(gl: Rc<Gl>) -> anyhow::Result<Self> {
        Ok(Self {
            solid_renderer: SolidRenderer::new(gl)?,
            shape: None,
            current_vp: None,
            current_dimensions: None,
            current_grid_size: None,
            current_n_revelead: None,
        })
    }

    pub fn create_fog(&mut self, vp: Rect, grid_size: f32, fog: &scene::Fog) {
        let mut points = PointVector::new();

        let d = grid_size;

        let mut fill_tile = |x, y| {
            let x = (x as f32) * d - vp.x;
            let y = (y as f32) * d - vp.y;

            let tl = Point::new(x, y);
            let tr = Point::new(x + d, y);
            let bl = Point::new(x, y + d);
            let br = Point::new(x + d, y + d);

            points.add_tri(tl, tr, br);
            points.add_tri(tl, bl, br);
        };

        for x in 0..fog.w {
            for y in 0..fog.h {
                if fog.occluded(x, y) {
                    fill_tile(x, y);
                }
            }
        }

        self.current_vp = Some(vp);
        self.current_dimensions = Some((fog.w, fog.h));
        self.current_grid_size = Some(grid_size);
        self.current_n_revelead = Some(fog.n_revealed);

        if let Ok(mut mesh) = Mesh::new(
            &self.solid_renderer.gl,
            &self.solid_renderer.program,
            &points.data,
        ) {
            mesh.set_transforms(false, false);
            self.shape = Some(mesh);
        }
    }

    pub fn render_fog(&mut self, vp: Rect, grid_size: f32, fog: &scene::Fog, colour: Colour) {
        if self.shape.is_none()
            || self.current_vp.is_none()
            || self.current_vp.unwrap() != vp
            || self.current_dimensions.is_none()
            || self.current_dimensions.unwrap() != (fog.w, fog.h)
            || self.current_grid_size != Some(grid_size)
            || self.current_n_revelead != Some(fog.n_revealed)
        {
            self.create_fog(vp, grid_size, fog);
        }

        if let Some(shape) = self.shape.as_ref() {
            self.solid_renderer.draw(shape, colour, vp, vp);
        }
    }
}

fn create_shader(gl: &Gl, src: &str, stype: u32) -> anyhow::Result<WebGlShader> {
    let shader = match gl.create_shader(stype) {
        Some(s) => s,
        None => return Err(anyhow::anyhow!("Failed to create shader.")),
    };

    gl.shader_source(&shader, src);
    gl.compile_shader(&shader);

    if gl
        .get_shader_parameter(&shader, Gl::COMPILE_STATUS)
        .is_falsy()
    {
        return match gl.get_shader_info_log(&shader) {
            Some(e) => Err(anyhow::anyhow!("Shader compilation failed, log: {e}")),
            None => Err(anyhow::anyhow!(
                "Shader compilation failed, no error message."
            )),
        };
    }

    Ok(shader)
}

fn create_program(gl: &Gl, vert: &str, frag: &str) -> anyhow::Result<WebGlProgram> {
    let program = match gl.create_program() {
        Some(p) => p,
        None => return Err(anyhow::anyhow!("WebGL program creation failed.")),
    };

    gl.attach_shader(&program, &create_shader(gl, vert, Gl::VERTEX_SHADER)?);
    gl.attach_shader(&program, &create_shader(gl, frag, Gl::FRAGMENT_SHADER)?);

    gl.link_program(&program);

    if gl
        .get_program_parameter(&program, Gl::LINK_STATUS)
        .is_falsy()
    {
        gl.delete_program(Some(&program));
        return Err(anyhow::anyhow!("WebGL program linking failed."));
    }

    Ok(program)
}

fn create_buffer(gl: &Gl, data_opt: Option<&Float32Array>) -> anyhow::Result<WebGlBuffer> {
    let buffer = match gl.create_buffer() {
        Some(b) => b,
        None => return Err(anyhow::anyhow!("Failed to create WebGL buffer.")),
    };

    if let Some(data) = data_opt {
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&buffer));
        gl.buffer_data_with_opt_array_buffer(
            Gl::ARRAY_BUFFER,
            Some(&data.buffer()),
            Gl::STATIC_DRAW,
        );
    }

    Ok(buffer)
}

fn get_uniform_location(
    gl: &Gl,
    program: &WebGlProgram,
    location: &str,
) -> anyhow::Result<WebGlUniformLocation> {
    match gl.get_uniform_location(program, location) {
        Some(l) => Ok(l),
        None => Err(anyhow::anyhow!(
            "Failed to get WebGlUniformLocation {location}."
        )),
    }
}

// Map value (as a proportion of scale) to [-1, 1]
fn to_unit(value: f32, scale: f32) -> f32 {
    ((2.0 * value) - scale) / scale
}

// see https://webglfundamentals.org/webgl/resources/m4.js
fn m4_orthographic(l: f32, r: f32, b: f32, t: f32, n: f32, f: f32) -> [f32; 16] {
    [
        2.0 / (r - l),
        0.0,
        0.0,
        0.0,
        0.0,
        2.0 / (t - b),
        0.0,
        0.0,
        0.0,
        0.0,
        2.0 / (n - f),
        0.0,
        (l + r) / (l - r),
        (b + t) / (b - t),
        (n + f) / (n - f),
        1.0,
    ]
}

// Translates matrix m by tx units in the x direction and likewise for ty and tz.
// NB: in place
fn m4_translate(m: &mut [f32; 16], tx: f32, ty: f32, tz: f32) {
    m[12] += m[0] * tx + m[4] * ty + m[8] * tz;
    m[13] += m[1] * tx + m[5] * ty + m[9] * tz;
    m[14] += m[2] * tx + m[6] * ty + m[10] * tz;
    m[15] += m[3] * tx + m[7] * ty + m[11] * tz;
}

// NB: in place
fn m4_scale(m: &mut [f32; 16], sx: f32, sy: f32, sz: f32) {
    m[0] *= sx;
    m[1] *= sx;
    m[2] *= sx;
    m[3] *= sx;
    m[4] *= sy;
    m[5] *= sy;
    m[6] *= sy;
    m[7] *= sy;
    m[8] *= sz;
    m[9] *= sz;
    m[10] *= sz;
    m[11] *= sz;
}

fn m4_rotatez(m: &mut [f32; 16], theta: f32) {
    let m00 = m[0];
    let m01 = m[1];
    let m02 = m[2];
    let m03 = m[3];
    let m10 = m[4];
    let m11 = m[5];
    let m12 = m[6];
    let m13 = m[7];
    let c = theta.cos();
    let s = theta.sin();

    m[0] = c * m00 + s * m10;
    m[1] = c * m01 + s * m11;
    m[2] = c * m02 + s * m12;
    m[3] = c * m03 + s * m13;
    m[4] = c * m10 - s * m00;
    m[5] = c * m11 - s * m01;
    m[6] = c * m12 - s * m02;
    m[7] = c * m13 - s * m03;
}
