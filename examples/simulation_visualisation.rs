extern crate sdl2;
extern crate dust;

mod simulation_material;
mod foam_loader;

use std::process;

use sdl2::event::{Event};
use sdl2::keyboard::Keycode;

use dust::*;

fn main() {
    let ctx = sdl2::init().unwrap();
    let video_ctx = ctx.video().unwrap();

    #[cfg(target_os = "macos")] // Use OpenGL 4.1 since that is the newest version supported on macOS
    {
        let gl_attr = video_ctx.gl_attr();
        gl_attr.set_context_profile(sdl2::video::GLProfile::Core);
        gl_attr.set_context_version(4, 1);
    }

    let width = 900;
    let height = 700;
    let window = video_ctx
        .window("Dust", width, height)
        .opengl()
        .position_centered()
        .resizable()
        .build()
        .unwrap();

    let _gl_context = window.gl_create_context().unwrap();
    let gl = gl::Gl::load_with(|s| video_ctx.gl_get_proc_address(s) as *const std::os::raw::c_void);

    // Scene
    let mut scene = scene::Scene::create().unwrap();

    // Camera
    let mut camera = camera::Camera::create(&gl, glm::vec3(5.0, 5.0, 5.0), glm::vec3(0.0, 0.0, 0.0), width, height).unwrap();

    unsafe {
        gl.ClearColor(0.3, 0.3, 0.5, 1.0);
    }

    // set up event handling
    let mut events = ctx.event_pump().unwrap();

    // main loop
    let main_loop = || {
        for event in events.poll_iter() {
            match event {
                Event::Quit {..} | Event::KeyDown {keycode: Some(Keycode::Escape), ..} => {
                    process::exit(1);
                },
                Event::KeyDown {keycode: Some(Keycode::R), ..} =>
                {
                    add_model_from_foam(&mut scene, &gl);
                },
                Event::MouseMotion {xrel, yrel, mousestate, .. } => {
                    if mousestate.left()
                    {
                        eventhandler::rotate(&mut camera, xrel, yrel);
                    }
                },
                Event::MouseWheel {y, .. } => {
                    eventhandler::zoom(&mut camera, y);
                },
                _ => {}
            }
        }

        // draw
        camera.draw(&scene).unwrap();

        window.gl_swap_window();
    };

    renderer::set_main_loop(main_loop);
}

fn add_model_from_foam(scene: &mut scene::Scene, gl: &gl::Gl)
{
    foam_loader::load("user/openfoam/constant/polyMesh/points", |points: Vec<f32>| {
        foam_loader::load("user/openfoam/constant/polyMesh/faces", |faces: Vec<u32>| {
            foam_loader::load("user/openfoam/constant/polyMesh/owner", |owner: Vec<u32>| {
                foam_loader::load("user/openfoam/constant/polyMesh/neighbour", |neighbour: Vec<u32>| {

                    let mesh = create_mesh(&points, &faces, &owner, &neighbour);
                    let cells = create_cell_data(&owner, &neighbour);
                    let material = simulation_material::SimulationMaterial::create(&gl, &points, &faces, &cells).unwrap();
                    scene.add_model(&gl, mesh, material).unwrap();
                });
            });
        });
    });
}

fn create_cell_data(owner: &Vec<u32>, neighbour: &Vec<u32>) -> Vec<u32>
{
    let no_cells = *owner.iter().max().unwrap() as usize + 1;
    println!("{}", no_cells);
    let mut cells = Vec::new();
    use std::iter;
    cells.extend(iter::repeat(0 as u32).take(4 * no_cells));

    let mut cell_count = Vec::new();
    cell_count.extend(iter::repeat(0).take(no_cells));

    for face_id in 0..owner.len() {
        let cell_id = owner[face_id] as usize;
        cells[ cell_id * 4 + cell_count[cell_id] ] = face_id as u32;
        cell_count[cell_id] = cell_count[cell_id] + 1;
    }

    for face_id in 0..neighbour.len() {
        let cell_id = neighbour[face_id] as usize;
        cells[ cell_id * 4 + cell_count[cell_id] ] = face_id as u32;
        cell_count[cell_id] = cell_count[cell_id] + 1;
    }
    println!("{:?}", cells);
    cells
}

fn create_mesh(positions: &Vec<f32>, faces: &Vec<u32>, owners: &Vec<u32>, neighbours: &Vec<u32>) -> mesh::Mesh
{
    let mut boundary_vertices = Vec::new();
    let mut boundary_face_ids = Vec::new();
    let mut boundary_face_id = 0;
    for face_id in neighbours.len()..owners.len()
    {
        for k in 0..3
        {
            let index = faces[face_id * 3 + k] as usize;
            boundary_vertices.push(positions[3 * index]);
            boundary_vertices.push(positions[3 * index + 1]);
            boundary_vertices.push(positions[3 * index + 2]);

            boundary_face_ids.push(boundary_face_id);
            boundary_face_id = boundary_face_id + 1;
        }
    }
    println!("{:?}", boundary_face_ids);
    println!("{:?}", boundary_vertices);
    let mut mesh = mesh::Mesh::create_unsafe(boundary_face_ids.clone(), &boundary_vertices).unwrap();
    mesh.add_custom_int_attribute("FaceId", &boundary_face_ids);
    mesh
}