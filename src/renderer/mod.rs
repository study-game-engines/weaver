use crate::{
    core::{
        camera::PerspectiveCamera,
        color::Color,
        light::{Light, PointLight},
        mesh::Mesh,
        transform::Transform,
    },
    ecs::world::World,
};

use rayon::prelude::*;

use self::shader::FragmentShader;
#[macro_use]
pub mod shader;
mod render_impl;

#[derive(Debug, Clone, Copy)]
pub struct Pixel {
    pub x: i32,
    pub y: i32,
    pub color: Color,
    pub depth: f32,
    pub normal: glam::Vec3A,
    pub position: glam::Vec3A,
}

pub struct Renderer {
    screen_width: usize,
    screen_height: usize,
    color_buffer: Vec<Color>,
    depth_buffer: Vec<f32>,
    normal_buffer: Vec<glam::Vec3A>,
    position_buffer: Vec<glam::Vec3A>,
    camera: PerspectiveCamera,
}

impl Renderer {
    pub fn new(screen_width: usize, screen_height: usize) -> Self {
        let mut camera = PerspectiveCamera::new(
            glam::Vec3A::new(4.0, 4.0, 4.0),
            glam::Vec3A::ZERO,
            120.0f32.to_radians(),
            screen_width as f32 / screen_height as f32,
            0.001,
            100000.0,
        );
        camera.look_at(
            glam::Vec3A::new(4.0, 4.0, 4.0),
            glam::Vec3A::new(0.0, 0.0, 0.0),
            glam::Vec3A::NEG_Y,
        );
        Self {
            screen_width,
            screen_height,
            color_buffer: vec![Color::new(0.0, 0.0, 0.0); screen_width * screen_height],
            depth_buffer: vec![f32::INFINITY; screen_width * screen_height],
            normal_buffer: vec![glam::Vec3A::ZERO; screen_width * screen_height],
            position_buffer: vec![glam::Vec3A::ZERO; screen_width * screen_height],
            camera,
        }
    }

    #[inline]
    pub fn color_buffer(&self) -> &[Color] {
        &self.color_buffer
    }

    #[inline]
    pub fn clear(&mut self, color: Color) {
        self.color_buffer.fill(color);
        self.depth_buffer.fill(f32::INFINITY);
        self.normal_buffer.fill(glam::Vec3A::ZERO);
        self.position_buffer.fill(glam::Vec3A::ZERO);
    }

    #[inline]
    pub fn set_color(&mut self, x: usize, y: usize, color: Color) {
        if x >= self.screen_width || y >= self.screen_height {
            return;
        }
        let index = y * self.screen_width + x;
        self.color_buffer[index] = color;
    }

    #[inline]
    pub fn get_color(&self, x: usize, y: usize) -> Color {
        if x >= self.screen_width || y >= self.screen_height {
            return Color::BLACK;
        }
        let index = y * self.screen_width + x;
        self.color_buffer[index]
    }

    #[inline]
    pub fn set_depth(&mut self, x: usize, y: usize, depth: f32) {
        if x >= self.screen_width || y >= self.screen_height {
            return;
        }
        let index = y * self.screen_width + x;
        self.depth_buffer[index] = depth;
    }

    #[inline]
    pub fn get_depth(&self, x: usize, y: usize) -> f32 {
        if x >= self.screen_width || y >= self.screen_height {
            return f32::INFINITY;
        }
        let index = y * self.screen_width + x;
        self.depth_buffer[index]
    }

    #[inline]
    pub fn set_normal(&mut self, x: usize, y: usize, normal: glam::Vec3A) {
        if x >= self.screen_width || y >= self.screen_height {
            return;
        }
        let index = y * self.screen_width + x;
        self.normal_buffer[index] = normal;
    }

    #[inline]
    pub fn get_normal(&self, x: usize, y: usize) -> glam::Vec3A {
        if x >= self.screen_width || y >= self.screen_height {
            return glam::Vec3A::ZERO;
        }
        let index = y * self.screen_width + x;
        self.normal_buffer[index]
    }

    #[inline]
    pub fn set_position(&mut self, x: usize, y: usize, position: glam::Vec3A) {
        if x >= self.screen_width || y >= self.screen_height {
            return;
        }
        let index = y * self.screen_width + x;
        self.position_buffer[index] = position;
    }

    #[inline]
    pub fn get_position(&self, x: usize, y: usize) -> glam::Vec3A {
        if x >= self.screen_width || y >= self.screen_height {
            return glam::Vec3A::ZERO;
        }
        let index = y * self.screen_width + x;
        self.position_buffer[index]
    }

    #[inline]
    pub fn camera(&self) -> &PerspectiveCamera {
        &self.camera
    }

    #[inline]
    pub fn screen_width(&self) -> usize {
        self.screen_width
    }

    #[inline]
    pub fn screen_height(&self) -> usize {
        self.screen_height
    }

    #[inline]
    pub fn view_to_screen(&self, (x, y): (f32, f32)) -> (i32, i32) {
        let x = (x + 1.0) / 2.0 * self.screen_width as f32;
        let y = (y + 1.0) / 2.0 * self.screen_height as f32;
        (x as i32, y as i32)
    }

    #[inline]
    pub fn screen_to_view(&self, (x, y): (usize, usize)) -> (f32, f32) {
        let x = x as f32 / self.screen_width as f32 * 2.0 - 1.0;
        let y = y as f32 / self.screen_height as f32 * 2.0 - 1.0;
        (x, y)
    }

    pub fn render(&mut self, world: &mut World) {
        self.clear(Color::new(0.1, 0.1, 0.1));

        // query the world for entities that have both a mesh and transform
        let query = world.read::<(Mesh, Transform)>();

        // rasterize each mesh
        let mut all_pixels = Vec::new();
        for (mesh, transform) in query
            .get::<Mesh>()
            .into_iter()
            .zip(query.get::<Transform>())
        {
            let pixels = (0..mesh.indices.len())
                .into_par_iter()
                .step_by(3)
                .map(|i| {
                    let i0 = mesh.indices[i] as usize;
                    let i1 = mesh.indices[i + 1] as usize;
                    let i2 = mesh.indices[i + 2] as usize;

                    let v0 = mesh.vertices[i0];
                    let v1 = mesh.vertices[i1];
                    let v2 = mesh.vertices[i2];

                    self.triangle(
                        v0,
                        v1,
                        v2,
                        &vertex_shader!(shader::TransformVertexShader {
                            transform: *transform,
                        }),
                        mesh.texture.as_ref(),
                    )
                })
                .flatten()
                .collect::<Vec<_>>();
            all_pixels.extend(pixels);
        }

        for pixel in all_pixels {
            if self.get_depth(pixel.x as usize, pixel.y as usize) > pixel.depth {
                self.set_color(pixel.x as usize, pixel.y as usize, pixel.color);
                self.set_depth(pixel.x as usize, pixel.y as usize, pixel.depth);
                self.set_normal(pixel.x as usize, pixel.y as usize, pixel.normal);
                self.set_position(pixel.x as usize, pixel.y as usize, pixel.position);
            }
        }

        let lights: Vec<Light> = world
            .read::<Light>()
            .get::<Light>()
            .iter()
            .copied()
            .copied()
            .collect();

        // lighting pass
        let mut lighting_buffers = vec![vec![Color::BLACK; self.color_buffer.len()]; lights.len()];

        lighting_buffers
            .par_iter_mut()
            .enumerate()
            .for_each(|(i, buffer)| {
                let light = lights[i];
                let shader = shader::PhongFragmentShader {
                    light,
                    camera_position: self.camera.position(),
                    shininess: 1.0,
                };
                buffer.par_iter_mut().enumerate().for_each(|(i, color)| {
                    let normal = self.normal_buffer[i];
                    let position = self.position_buffer[i];
                    let depth = self.depth_buffer[i];

                    *color = shader.fragment_shader(position, normal, depth, *color);
                });
            });

        // combine lighting buffers
        let unlit_color_buffer = self.color_buffer.clone();
        self.color_buffer.fill(Color::BLACK);
        for buffer in lighting_buffers.iter() {
            self.color_buffer
                .par_iter_mut()
                .zip(buffer.par_iter())
                .for_each(|(color, light_color)| {
                    *color += *light_color;
                });
        }

        // multiply by unlit color
        self.color_buffer
            .par_iter_mut()
            .zip(unlit_color_buffer.par_iter())
            .for_each(|(color, unlit_color)| {
                *color *= *unlit_color;
            });

        // // gamma correct
        // for color in self.color_buffer.iter_mut() {
        //     *color = color.gamma_corrected(2.2).clamp(0.0, 1.0);
        // }
    }
}
