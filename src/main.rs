use crate::{x11comm::{connect_x11_socket, x11_create_graphical_context, load_x11_auth_token, next_x11_id, x11_handshake, x11_create_window, x11_map_window, x11_create_pixmap, x11_put_image, x11_copy_area},
            config::{ENTITIES_COLUMN_COUNT, ENTITIES_ROW_COUNT, ENTITIES_WIDTH, ENTITIES_HEIGHT},
            game::Scene};
use png;
use std::fs::File;
use std::thread::sleep;
use std::time;
use crate::game::SceneState;
use crate::utils::rgba_to_bgra;

mod x11comm;
mod utils;
mod game;
mod config;


fn main() {
    let auth_token = load_x11_auth_token().unwrap();
    let mut socket = connect_x11_socket().unwrap();
    let connection_information = x11_handshake(&mut socket, &auth_token).unwrap();
    println!("{:#?}", connection_information);

    let gc_id = next_x11_id(0, connection_information);
    x11_create_graphical_context(&mut socket, gc_id, connection_information.root_screen.id);

    let window_id = next_x11_id(gc_id, connection_information);
    x11_create_window(
        &mut socket,
        window_id,
        connection_information.root_screen.id,
        200,
        200,
        (ENTITIES_COLUMN_COUNT * ENTITIES_WIDTH) as u16,
        (ENTITIES_ROW_COUNT * ENTITIES_HEIGHT) as u16,
        connection_information.root_screen.root_visual_id,
    );

    x11_map_window(&mut socket, window_id);

    let decoder = png::Decoder::new(File::open("resources/img.png").unwrap());
    let mut reader = decoder.read_info().unwrap();
    let mut pngbuf = vec![0; reader.output_buffer_size()];
    let pngoutputinfo = reader.next_frame(&mut pngbuf).unwrap();
    let pngbytes = &pngbuf[..pngoutputinfo.buffer_size()];
    let x11_sprite_bytes = rgba_to_bgra(pngbytes);

    let pixmap_id = next_x11_id(window_id, connection_information);
    x11_create_pixmap(
        &mut socket,
        window_id,
        pixmap_id,
        pngoutputinfo.width as u16,
        pngoutputinfo.height as u16,
        24,
    );


    x11_put_image(
        &mut socket,
        window_id,
        pixmap_id,
        gc_id,
        pngoutputinfo.width as u16,
        pngoutputinfo.height as u16,
        0,
        0,
        24,
        x11_sprite_bytes,
    );
    // TODO: figure out a way to get if the socket is empty or not
    sleep(time::Duration::from_millis(75));

    let mut scene = Scene::new(window_id, gc_id, pixmap_id);
    scene.reset();
    scene.render(&mut socket);
    scene.wait_for_x11_events(socket);
}
