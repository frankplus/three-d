
use crate::camera;
use crate::light;
use crate::*;
use crate::objects::FullScreen;

#[derive(Debug)]
pub enum Error {
    IO(std::io::Error),
    Program(program::Error),
    Rendertarget(rendertarget::Error),
    Texture(texture::Error),
    LightPassRendertargetNotAvailable {message: String}
}

impl From<std::io::Error> for Error {
    fn from(other: std::io::Error) -> Self {
        Error::IO(other)
    }
}

impl From<program::Error> for Error {
    fn from(other: program::Error) -> Self {
        Error::Program(other)
    }
}

impl From<rendertarget::Error> for Error {
    fn from(other: rendertarget::Error) -> Self {
        Error::Rendertarget(other)
    }
}

impl From<texture::Error> for Error {
    fn from(other: texture::Error) -> Self {
        Error::Texture(other)
    }
}

pub struct DeferredPipeline {
    gl: Gl,
    light_pass_program: program::Program,
    rendertarget: rendertarget::ColorRendertarget,
    geometry_pass_rendertarget: rendertarget::ColorRendertarget,
    full_screen: FullScreen,
    pub background_color: Vec4
}


impl DeferredPipeline
{
    pub fn new(gl: &Gl, screen_width: usize, screen_height: usize, background_color: Vec4) -> Result<DeferredPipeline, Error>
    {
        let light_pass_program = program::Program::from_source(&gl,
                                                               include_str!("shaders/light_pass.vert"),
                                                               include_str!("shaders/light_pass.frag"))?;
        let rendertarget = rendertarget::ColorRendertarget::default(gl, screen_width, screen_height)?;
        let geometry_pass_rendertarget = rendertarget::ColorRendertarget::new(&gl, screen_width, screen_height, 4)?;
        Ok(DeferredPipeline { gl: gl.clone(), light_pass_program, rendertarget, geometry_pass_rendertarget, full_screen: FullScreen::new(gl), background_color })
    }

    pub fn resize(&mut self, screen_width: usize, screen_height: usize) -> Result<(), Error>
    {
        self.rendertarget.width = screen_width;
        self.rendertarget.height = screen_height;
        self.geometry_pass_rendertarget = rendertarget::ColorRendertarget::new(&self.gl, screen_width, screen_height, 4)?;
        Ok(())
    }

    pub fn geometry_pass_begin(&self) -> Result<(), Error>
    {
        self.geometry_pass_rendertarget.bind();
        self.geometry_pass_rendertarget.clear(&self.background_color);

        state::depth_write(&self.gl, true);
        state::depth_test(&self.gl, state::DepthTestType::LEQUAL);
        state::cull(&self.gl, state::CullType::NONE);
        state::blend(&self.gl, state::BlendType::NONE);

        Ok(())
    }

    pub fn light_pass_begin(&self, camera: &camera::Camera) -> Result<(), Error>
    {
        self.light_pass_render_to(camera, &self.rendertarget)?;
        Ok(())
    }

    pub fn light_pass_render_to(&self, camera: &camera::Camera, rendertarget: &ColorRendertarget) -> Result<(), Error>
    {
        rendertarget.bind();
        rendertarget.clear(&vec4(0.0, 0.0, 0.0, 0.0));

        state::depth_write(&self.gl,false);
        state::depth_test(&self.gl, state::DepthTestType::NONE);
        state::cull(&self.gl,state::CullType::BACK);
        state::blend(&self.gl, state::BlendType::ONE__ONE);

        self.geometry_pass_color_texture().bind(0);
        self.light_pass_program.add_uniform_int("colorMap", &0)?;

        self.geometry_pass_position_texture().bind(1);
        self.light_pass_program.add_uniform_int("positionMap", &1)?;

        self.geometry_pass_normal_texture().bind(2);
        self.light_pass_program.add_uniform_int("normalMap", &2)?;

        self.geometry_pass_surface_parameters_texture().bind(3);
        self.light_pass_program.add_uniform_int("surfaceParametersMap", &3)?;

        self.geometry_pass_depth_texture().bind(4);
        self.light_pass_program.add_uniform_int("depthMap", &4)?;

        self.light_pass_program.add_uniform_vec3("eyePosition", &camera.position())?;

        Ok(())
    }

    pub fn shine_ambient_light(&self, light: &light::AmbientLight) -> Result<(), Error>
    {
        self.light_pass_program.add_uniform_int("lightType", &0)?;
        self.light_pass_program.add_uniform_vec3("ambientLight.base.color", &light.base.color)?;
        self.light_pass_program.add_uniform_float("ambientLight.base.intensity", &light.base.intensity)?;

        self.full_screen.render(&self.light_pass_program);
        Ok(())
    }

    pub fn shine_directional_light(&self, light: &light::DirectionalLight) -> Result<(), Error>
    {
        if let Ok(shadow_camera) = light.shadow_camera() {
            use crate::camera::Camera;
            let bias_matrix = crate::Mat4::new(
                                 0.5, 0.0, 0.0, 0.0,
                                 0.0, 0.5, 0.0, 0.0,
                                 0.0, 0.0, 0.5, 0.0,
                                 0.5, 0.5, 0.5, 1.0);
            self.light_pass_program.add_uniform_mat4("shadowMVP", &(bias_matrix * *shadow_camera.get_projection() * *shadow_camera.get_view()))?;

            light.shadow_rendertarget.as_ref().unwrap().target.bind(5);
            self.light_pass_program.add_uniform_int("shadowMap", &5)?;
        }

        //self.light_pass_program.add_uniform_int("shadowCubeMap", &6)?;

        self.light_pass_program.add_uniform_int("lightType", &1)?;
        self.light_pass_program.add_uniform_vec3("directionalLight.direction", &light.direction)?;
        self.light_pass_program.add_uniform_vec3("directionalLight.base.color", &light.base.color)?;
        self.light_pass_program.add_uniform_float("directionalLight.base.intensity", &light.base.intensity)?;

        self.full_screen.render(&self.light_pass_program);
        Ok(())
    }

    pub fn shine_point_light(&self, light: &light::PointLight) -> Result<(), Error>
    {
        //self.light_pass_program.add_uniform_int("shadowMap", &5)?;
        //self.light_pass_program.add_uniform_int("shadowCubeMap", &6)?;

        self.light_pass_program.add_uniform_int("lightType", &2)?;
        self.light_pass_program.add_uniform_vec3("pointLight.position", &light.position)?;
        self.light_pass_program.add_uniform_vec3("pointLight.base.color", &light.base.color)?;
        self.light_pass_program.add_uniform_float("pointLight.base.intensity", &light.base.intensity)?;
        self.light_pass_program.add_uniform_float("pointLight.attenuation.constant", &light.attenuation.constant)?;
        self.light_pass_program.add_uniform_float("pointLight.attenuation.linear", &light.attenuation.linear)?;
        self.light_pass_program.add_uniform_float("pointLight.attenuation.exp", &light.attenuation.exp)?;

        self.full_screen.render(&self.light_pass_program);
        Ok(())
    }

    pub fn shine_spot_light(&self, light: &light::SpotLight) -> Result<(), Error>
    {
        if let Ok(shadow_camera) = light.shadow_camera() {
            use crate::camera::Camera;
            let bias_matrix = crate::Mat4::new(
                                 0.5, 0.0, 0.0, 0.0,
                                 0.0, 0.5, 0.0, 0.0,
                                 0.0, 0.0, 0.5, 0.0,
                                 0.5, 0.5, 0.5, 1.0);
            self.light_pass_program.add_uniform_mat4("shadowMVP", &(bias_matrix * *shadow_camera.get_projection() * *shadow_camera.get_view()))?;

            light.shadow_rendertarget.as_ref().unwrap().target.bind(5);
            self.light_pass_program.add_uniform_int("shadowMap", &5)?;
        }

        //self.light_pass_program.add_uniform_int("shadowCubeMap", &6)?;

        self.light_pass_program.add_uniform_int("lightType", &3)?;
        self.light_pass_program.add_uniform_vec3("spotLight.position", &light.position)?;
        self.light_pass_program.add_uniform_vec3("spotLight.direction", &light.direction)?;
        self.light_pass_program.add_uniform_vec3("spotLight.base.color", &light.base.color)?;
        self.light_pass_program.add_uniform_float("spotLight.base.intensity", &light.base.intensity)?;
        self.light_pass_program.add_uniform_float("spotLight.attenuation.constant", &light.attenuation.constant)?;
        self.light_pass_program.add_uniform_float("spotLight.attenuation.linear", &light.attenuation.linear)?;
        self.light_pass_program.add_uniform_float("spotLight.attenuation.exp", &light.attenuation.exp)?;
        self.light_pass_program.add_uniform_float("spotLight.cutoff", &light.cutoff.cos())?;

        self.full_screen.render(&self.light_pass_program);
        Ok(())
    }

    pub fn full_screen(&self) -> &FullScreen
    {
        &self.full_screen
    }

    pub fn screen_rendertarget(&self) -> &ColorRendertarget
    {
        &self.rendertarget
    }

    pub fn geometry_pass_color_texture(&self) -> &Texture
    {
        &self.geometry_pass_rendertarget.targets[0]
    }

    pub fn geometry_pass_position_texture(&self) -> &Texture
    {
        &self.geometry_pass_rendertarget.targets[1]
    }

    pub fn geometry_pass_normal_texture(&self) -> &Texture
    {
        &self.geometry_pass_rendertarget.targets[2]
    }

    pub fn geometry_pass_surface_parameters_texture(&self) -> &Texture
    {
        &self.geometry_pass_rendertarget.targets[3]
    }

    pub fn geometry_pass_depth_texture(&self) -> &Texture
    {
        self.geometry_pass_rendertarget.depth_target.as_ref().unwrap()
    }
}