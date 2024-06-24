use std::io::{self, Read, Cursor, Write};
use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use std::{env, process};
use std::path::PathBuf;
use std::fs;
use std::mem::size_of;
use std::os::unix::net::UnixStream;

const AUTH_ENTRY_FAMILY_LOCAL: u16 = 1;
const AUTH_ENTRY_MAGIC_COOKIE: &str = "MIT-MAGIC-COOKIE-1";

type AuthToken = [u8; 16];

#[derive(Debug)]
struct AuthEntry {
    family: u16,
    address: Vec<u8>,
    display_number: String,
    auth_name: String,
    auth_data: Vec<u8>,
}

#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct Screen {
    pub id: u32,
    colormap: u32,
    white: u32,
    black: u32,
    input_mask: u32,
    width: u16,
    height: u16,
    width_mm: u16,
    height_mm: u16,
    maps_min: u16,
    maps_max: u16,
    pub(crate) root_visual_id: u32,
    backing_store: u8,
    save_unders: u8,
    root_depth: u8,
    depths_count: u8,
}

#[derive(Debug, Copy, Clone)]
pub struct ConnectionInformation {
    pub root_screen: Screen,
    pub resource_id_base: u32,
    pub resource_id_mask: u32,
}

#[repr(C, packed)]
struct HandshakeRequest {
    endianness: u8,
    pad1: u8,
    major_version: u16,
    minor_version: u16,
    authorization_len: u16,
    authorization_data_len: u16,
    pad2: u16,
}

#[repr(C, packed)]
struct GraphicalContextRequest {
    opcode:   u8,
    pad1:     u8,
    length:   u16,
    id:       u32,
    drawable: u32,
    bitmask:  u32,
    value1:   u32,
}

#[repr(C, packed)]
struct CreateWindowRequest {
    opcode:         u8,
    depth:          u8,
    request_length: u16,
    window_id:      u32,
    parent_id:      u32,
    x:              u16,
    y:              u16,
    width:          u16,
    height:         u16,
    border_width:   u16,
    class:          u16,
    root_visual_id: u32,
    bitmask:        u32,
    value1:         u32,
    value2:         u32,
}

#[repr(C, packed)]
struct MapWindowRequest {
    opcode: u8,
    pad1: u8,
    request_length: u16,
    window_id: u32,
}

#[repr(C, packed)]
struct CreatePixmapRequest {
    opcode:         u8,
    depth:          u8,
    request_length: u16,
    pixmap_id:      u32,
    drawable_id:    u32,
    width:          u16,
    height:         u16,
}

#[repr(C, packed)]
struct PutImageRequest {
    opcode:         u8,
    format:         u8,
    request_length: u16,
    drawable_id:    u32,
    gc_id:          u32,
    width:          u16,
    height:         u16,
    dst_x:          u16,
    dst_y:          u16,
    left_pad:       u8,
    depth:          u8,
    pad1:           u16,
}

#[repr(C, packed)]
struct CopyAreaRequest {
    opcode:         u8,
    pad1:           u8,
    request_length: u16,
    src_id:         u32,
    dst_id:         u32,
    gc_id:          u32,
    src_x:          u16,
    src_y:          u16,
    dst_x:          u16,
    dst_y:          u16,
    width:          u16,
    height:         u16,
}

#[repr(C, packed)]
struct StaticResponse {
    success: u8,
    pad1: u8,
    major_version: u16,
    minor_version: u16,
    length: u16,
}

#[repr(C, packed)]
struct DynamicResponse {
    release_number: u32,
    resource_id_base: u32,
    resource_id_mask: u32,
    motion_buffer_size: u32,
    vendor_length: u16,
    maximum_request_length: u16,
    screens_in_root_count: u8,
    formats_count: u8,
    image_byte_order: u8,
    bitmap_format_bit_order: u8,
    bitmap_format_scanline_unit: u8,
    bitmap_format_scanline_pad: u8,
    min_keycode: u8,
    max_keycode: u8,
    pad2: u32,
}


fn read_x11_auth_entry(buffer: &mut Cursor<Vec<u8>>) -> io::Result<Option<AuthEntry>> {
    let family = buffer.read_u16::<LittleEndian>()?;

    let address_len = buffer.read_u16::<BigEndian>()?;
    let mut address = vec![0u8; address_len as usize];
    buffer.read_exact(&mut address)?;

    let display_number_len = buffer.read_u16::<BigEndian>()?;
    let mut display_number = vec![0u8; display_number_len as usize];
    buffer.read_exact(&mut display_number)?;
    let display_number = String::from_utf8_lossy(&display_number).to_string();

    let auth_name_len = buffer.read_u16::<BigEndian>()?;
    let mut auth_name = vec![0u8; auth_name_len as usize];
    buffer.read_exact(&mut auth_name)?;
    let auth_name = String::from_utf8_lossy(&auth_name).to_string();

    let auth_data_len = buffer.read_u16::<BigEndian>()?;
    let mut auth_data = vec![0u8; auth_data_len as usize];
    buffer.read_exact(&mut auth_data)?;

    Ok(Some(AuthEntry {
        family,
        address,
        display_number,
        auth_name,
        auth_data,
    }))
}

pub(crate) fn load_x11_auth_token() -> io::Result<AuthToken> {
    let filename = env::var("XAUTHORITY").unwrap_or_else(|_| {
        let home = env::var("HOME").expect("HOME environment variable not set");
        PathBuf::from(home).join(".Xauthority").to_str().unwrap().to_string()
    });

    let data = fs::read(&filename)?;
    let mut buffer = Cursor::new(data);

    while let Ok(Some(auth_entry)) = read_x11_auth_entry(&mut buffer) {
        if auth_entry.family == AUTH_ENTRY_FAMILY_LOCAL
            && auth_entry.auth_name == AUTH_ENTRY_MAGIC_COOKIE
            && auth_entry.auth_data.len() == std::mem::size_of::<AuthToken>()
        {
            let mut token = [0u8; 16];
            token.copy_from_slice(&auth_entry.auth_data);
            return Ok(token);
        }
    }

    Err(io::Error::new(io::ErrorKind::NotFound, "No suitable X11 auth token found"))
}

pub(crate) fn connect_x11_socket() -> io::Result<UnixStream> {
    let possible_socket_paths = ["/tmp/.X11-unix/X0", "/tmp/.X11-unix/X1"];

    for &socket_path in &possible_socket_paths {
        match UnixStream::connect(socket_path) {
            Ok(stream) => return Ok(stream),
            Err(_) => continue,
        }
    }

    eprintln!("Failed to connect to X11 socket");
    process::exit(1);
}

pub(crate) fn x11_handshake(socket: &mut UnixStream, auth_token: &AuthToken) -> Result<ConnectionInformation, std::io::Error> {
    let request = HandshakeRequest {
        endianness: b'l',
        pad1: 0,
        major_version: 11,
        minor_version: 0,
        authorization_len: AUTH_ENTRY_MAGIC_COOKIE.len() as u16,
        authorization_data_len: size_of::<AuthToken>() as u16,
        pad2: 0,
    };

    let padding = [0u8; 2];

    socket.write_all(unsafe { std::slice::from_raw_parts(&request as *const _ as *const u8, size_of::<HandshakeRequest>()) })?;
    socket.write_all(AUTH_ENTRY_MAGIC_COOKIE.as_bytes())?;
    socket.write_all(&padding)?;
    socket.write_all(auth_token)?;

    let mut static_response = StaticResponse { success: 0, pad1: 0, major_version: 0, minor_version: 0, length: 0 };
    socket.read_exact(unsafe { std::slice::from_raw_parts_mut(&mut static_response as *mut _ as *mut u8, size_of::<StaticResponse>()) })?;

    assert_eq!(static_response.success, 1);

    let mut recv_buf = vec![0u8; static_response.length as usize * 4];
    socket.read_exact(&mut recv_buf)?;

    let mut dynamic_response = DynamicResponse {
        release_number: 0, resource_id_base: 0, resource_id_mask: 0, motion_buffer_size: 0,
        vendor_length: 0, maximum_request_length: 0, screens_in_root_count: 0, formats_count: 0,
        image_byte_order: 0, bitmap_format_bit_order: 0, bitmap_format_scanline_unit: 0,
        bitmap_format_scanline_pad: 0, min_keycode: 0, max_keycode: 0, pad2: 0,
    };
    let dynamic_response_slice = unsafe { std::slice::from_raw_parts_mut(&mut dynamic_response as *mut _ as *mut u8, size_of::<DynamicResponse>()) };
    dynamic_response_slice.copy_from_slice(&recv_buf[..size_of::<DynamicResponse>()]);

    let vendor_length_padded = round_up_4(dynamic_response.vendor_length as u32) as usize;
    let formats_length = 8 * dynamic_response.formats_count as usize;
    let screen_offset = size_of::<DynamicResponse>() + vendor_length_padded + formats_length;

    let mut screen = Screen {
        id: 0, colormap: 0, white: 0, black: 0, input_mask: 0,
        width: 0, height: 0, width_mm: 0, height_mm: 0,
        maps_min: 0, maps_max: 0, root_visual_id: 0,
        backing_store: 0, save_unders: 0, root_depth: 0, depths_count: 0,
    };
    let screen_slice = unsafe { std::slice::from_raw_parts_mut(&mut screen as *mut _ as *mut u8, size_of::<Screen>()) };
    screen_slice.copy_from_slice(&recv_buf[screen_offset..screen_offset + size_of::<Screen>()]);

    Ok(ConnectionInformation {
        resource_id_base: dynamic_response.resource_id_base,
        resource_id_mask: dynamic_response.resource_id_mask,
        root_screen: screen,
    })
}

fn round_up_4(n: u32) -> u32 {
    (n + 3) & !3
}

pub(crate) fn next_x11_id(current_id: u32, info: ConnectionInformation) -> u32 {
    return 1 + ((info.resource_id_mask & (current_id)) | info.resource_id_base)
}

pub(crate) fn x11_create_graphical_context(socket: &mut UnixStream, gc_id: u32, root_id: u32) {
    const OPCODE: u8 = 55;
    const FLAG_GC_BG: u32 = 8;
    const BITMASK: u32 = FLAG_GC_BG;
    const VALUE1: u32 = 0x00_00_ff_00;
    
    let request = GraphicalContextRequest {
        opcode:   OPCODE,
        pad1:     0,
        length:   5,
        id:       gc_id,
        drawable: root_id,
        bitmask:  BITMASK,
        value1:   VALUE1,
    };

    return socket.write_all(unsafe { std::slice::from_raw_parts(&request as *const _ as *const u8, size_of::<GraphicalContextRequest>()) }).unwrap()
}

pub(crate) fn x11_create_window(
    socket: &mut UnixStream,
    window_id: u32,
    parent_id: u32,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    root_visual_id: u32,
){
    const FLAG_WIN_BG_PIXEL: u32 = 2;
    const FLAG_WIN_EVENT: u32 = 0x800;
    const FLAG_COUNT: u16 = 2;
    const EVENT_FLAG_EXPOSURE: u32 = 0x80_00;
    const EVENT_FLAG_KEY_PRESS: u32 = 0x1;
    const EVENT_FLAG_KEY_RELEASE: u32 = 0x2;
    const EVENT_FLAG_BUTTON_PRESS: u32 = 0x4;
    const EVENT_FLAG_BUTTON_RELEASE: u32 = 0x8;
    const FLAGS: u32 = FLAG_WIN_BG_PIXEL | FLAG_WIN_EVENT;
    const DEPTH: u8 = 24;
    const BORDER_WIDTH: u16 = 0;
    const CLASS_INPUT_OUTPUT: u16 = 1;
    const OPCODE: u8 = 1;
    const BACKGROUND_PIXEL_COLOR: u32 = 0x00_ff_ff_80;

    let request = CreateWindowRequest {
        opcode:          OPCODE,
        depth:           DEPTH,
        request_length:  8 + FLAG_COUNT,
        window_id:       window_id,
        parent_id:       parent_id,
        x:               x,
        y:               y,
        width:           width,
        height:          height,
        border_width:    BORDER_WIDTH,
        class:           CLASS_INPUT_OUTPUT,
        root_visual_id:  root_visual_id,
        bitmask:         FLAGS,
        value1:          BACKGROUND_PIXEL_COLOR,
        value2:          EVENT_FLAG_EXPOSURE | EVENT_FLAG_BUTTON_RELEASE | EVENT_FLAG_BUTTON_PRESS | EVENT_FLAG_KEY_PRESS | EVENT_FLAG_KEY_RELEASE,
    };
    return socket.write_all(unsafe { std::slice::from_raw_parts(&request as *const _ as *const u8, size_of::<CreateWindowRequest>()) }).unwrap()
}

pub(crate) fn x11_map_window(socket: &mut UnixStream, window_id: u32) {
    const OPCODE: u8 = 8;

    let request = MapWindowRequest {
        opcode: OPCODE,
        pad1: 0,
        request_length: 2,
        window_id: window_id,
    };

    return socket.write_all(unsafe { std::slice::from_raw_parts(&request as *const _ as *const u8, size_of::<MapWindowRequest>()) }).unwrap()
}

pub(crate) fn x11_create_pixmap(socket: &mut UnixStream,
                                window_id: u32,
                                pixmap_id: u32,
                                width: u16,
                                height: u16,
                                depth: u8) {
    const OPCODE: u8 = 53;

    let request = CreatePixmapRequest {
        opcode         : OPCODE,
        depth          : depth,
        request_length : 4,
        pixmap_id      : pixmap_id,
        drawable_id    : window_id,
        width          : width,
        height         : height,
    };

    return socket.write_all(unsafe { std::slice::from_raw_parts(&request as *const _ as *const u8, size_of::<CreatePixmapRequest>()) }).unwrap()
}

pub(crate) fn x11_put_image(socket: &mut UnixStream,
                                window_id: u32,
                                drawable_id: u32,
                                gc_id: u32,
                                width: u16,
                                height: u16,
                                dst_x: u16,
                                dst_y: u16,
                                depth: u8,
                                data: Vec<u8>,) {
    let data_length_padded = round_up_4(data.len() as u32);
    const OPCODE: u8 = 72;

    let request = PutImageRequest {
        opcode         : OPCODE,
        format         : 2, // ZPixmap
        request_length : (6 + data_length_padded / 4) as u16,
        drawable_id    : drawable_id,
        gc_id          : gc_id,
        width          : width,
        height         : height,
        dst_x          : dst_x,
        dst_y          : dst_y,
        left_pad       : 0,
        depth          : depth,
        pad1           : 0,
    };

    let padding_len = data_length_padded - data.len() as u32;
    println!("req length {:} = calculated {:}", ((6 + data_length_padded / 4) as u16), ((size_of::<PutImageRequest>()) + data.len() + padding_len as usize) / 4);
    socket.write_all(unsafe { std::slice::from_raw_parts(&request as *const _ as *const u8, size_of::<PutImageRequest>()) });
    socket.write_all(&*data);
    socket.write_all(&*vec![0u8; padding_len as usize]);
}

pub(crate) fn x11_copy_area(socket: &mut UnixStream,
                            src_id: u32,
                            dst_id: u32,
                            gc_id: u32,
                            src_x: u16,
                            src_y: u16,
                            dst_x: u16,
                            dst_y: u16,
                            width: u16,
                            height: u16) {
    const OPCODE: u8 = 62;

    let request = CopyAreaRequest {
        opcode         : OPCODE,
        pad1           : 0,
        request_length : 7,
        src_id         : src_id,
        dst_id         : dst_id,
        gc_id          : gc_id,
        src_x          : src_x,
        src_y          : src_y,
        dst_x          : dst_x,
        dst_y          : dst_y,
        width          : width,
        height         : height,
    };

    return socket.write_all(unsafe { std::slice::from_raw_parts(&request as *const _ as *const u8, size_of::<CopyAreaRequest>()) }).unwrap()
}
