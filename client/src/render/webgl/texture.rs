use std::{collections::HashMap, rc::Rc};

use scene::{Rect, Shape};
use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::{HtmlImageElement, WebGlBuffer, WebGlProgram, WebGlTexture, WebGlUniformLocation};

use super::{create_buffer, create_program, get_uniform_location, mesh::Mesh, Gl};
use crate::render::parse_media_key;

pub struct TextureRef<'a>(&'a WebGlTexture);

struct Texture {
    pub width: u32,
    pub height: u32,
    pub texture: WebGlTexture,
}

impl Texture {
    // 0 is the default and what is used here
    const GL_TEXTURE_DETAIL_LEVEL: i32 = 0;

    // Required to be 0 for textures
    const GL_TEXTURE_BORDER_WIDTH: i32 = 0;

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
                Self::GL_TEXTURE_DETAIL_LEVEL,
                Gl::RGBA as i32,
                width as i32,
                height as i32,
                Self::GL_TEXTURE_BORDER_WIDTH,
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
                Self::GL_TEXTURE_DETAIL_LEVEL,
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

        // Set callback to update texture once image is loaded. This is a memory
        // leak. Every time we load an image by URL we leak the memory for the
        // closure.
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
            Some(s) => parse_media_key(&s),
            None => 0,
        };

        if id != 0 {
            match Texture::from_html_image(&self.gl, image) {
                Ok(t) => self.textures.insert(id, t),
                Err(_) => return 0,
            };
        } else {
            crate::bridge::log("Texture manager was asked to load texture without ID.");
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
    pub fn get_texture(&mut self, id: scene::Id) -> TextureRef {
        if let Some(tex) = self.textures.get(&id) {
            TextureRef(&tex.texture)
        } else {
            if !self.loading.contains(&id) {
                self.loading.push(id);
                crate::bridge::load_texture(format!("{id:016X}"));
            }

            // This unwrap is safe because we always add a missing texture
            // texture as id 0 in the constructor.
            TextureRef(&self.textures.get(&0).unwrap().texture)
        }
    }
}

pub struct TextureShapeRenderer {
    gl: Rc<Gl>,
    program: WebGlProgram,
    texcoord_buffer: WebGlBuffer,
    texcoord_location: u32,
    texture_location: WebGlUniformLocation,
    shape: Mesh,
}

impl TextureShapeRenderer {
    pub fn new(gl: Rc<Gl>, shape: Shape) -> anyhow::Result<Self> {
        let program = create_program(
            &gl,
            include_str!("shaders/solid.vert"),
            include_str!("shaders/image.frag"),
        )?;

        let shape = Mesh::of_shape(&gl, &program, shape)?;

        let texcoord_location = gl.get_attrib_location(&program, "a_texcoord") as u32;
        let texcoord_buffer = create_buffer(&gl, Some(shape.points()))?;
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

    pub fn draw_texture(&self, texture: TextureRef, viewport: Rect, position: Rect) {
        let gl = &self.gl;

        gl.bind_texture(Gl::TEXTURE_2D, Some(texture.0));
        gl.use_program(Some(&self.program));
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&self.texcoord_buffer));
        gl.enable_vertex_attrib_array(self.texcoord_location);
        gl.vertex_attrib_pointer_with_i32(self.texcoord_location, 2, Gl::FLOAT, false, 0, 0);

        gl.uniform1i(Some(&self.texture_location), 0);
        self.shape.draw(gl, viewport, position);
    }
}
