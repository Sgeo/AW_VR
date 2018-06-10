#![crate_type="cdylib"]

extern crate easyhook;
extern crate libloading as lib;
extern crate openvr as vr;

#[macro_use]
extern crate lazy_static;

use std::io::Write;
use std::os::raw::{c_ulong, c_void};
use std::sync::atomic::{AtomicUsize, Ordering};
use easyhook::{lh_install_hook};
use easyhook::error_string;

lazy_static! {
    static ref GL: lib::Library = lib::Library::new("OPENGL32").unwrap();
    static ref RW: lib::Library = lib::Library::new("rw_opengl").unwrap();
    static ref glViewport: lib::Symbol<'static, extern "system" fn(i32, i32, u32, u32)> = unsafe { GL.get(b"glViewport\0") }.unwrap();
    static ref glGetIntegerv: lib::Symbol<'static, extern "system" fn(i32, *mut i32)> = unsafe { GL.get(b"glGetIntegerv\0") }.unwrap();
    static ref rw_camera_begin_update: lib::Symbol<'static, extern "C" fn(*mut c_void) -> *mut c_void> = unsafe { RW.get(b"rw_camera_begin_update\0") }.unwrap();
    static ref rw_camera_end_update: lib::Symbol<'static, extern "C" fn(*mut c_void) -> *mut c_void> = unsafe { RW.get(b"rw_camera_end_update\0") }.unwrap();
    static ref rw_frame_translate: lib::Symbol<'static, extern "C" fn(*mut c_void, *mut f32, u32) -> *mut c_void> = unsafe { RW.get(b"rw_frame_translate\0") }.unwrap();
    static ref rw_camera_set_view_window: lib::Symbol<'static, extern "C" fn(*mut c_void, *mut f32) -> *mut c_void> = unsafe { RW.get(b"rw_camera_set_view_window\0") }.unwrap();
}

thread_local! {
    // Use only for initializing SYSTEM and COMPOSITOR
    static CONTEXT: vr::Context = unsafe {
        std::fs::File::create("about_to_initialize_context.txt").unwrap();
        vr::init(vr::ApplicationType::Scene).unwrap()
    };
}

lazy_static! {
    static ref SYSTEM: vr::System = {
        CONTEXT.with(|context| context.system().unwrap())
    };
    static ref COMPOSITOR: vr::Compositor = {
        std::fs::File::create::<&'static str>("about_to_initialize_compositor.txt").unwrap();
        CONTEXT.with(|context| context.compositor().unwrap())
    };
}

#[export_name="_NativeInjectionEntryPoint_4"] // EasyHook32.dll has been hex edited to look for this
pub extern "stdcall" fn NativeInjectionEntryPoint(_remote_info: *mut c_void) {
    unsafe {
        use std::fs::File;
        File::create("about_to_install_hook.txt").unwrap();
        lazy_static::initialize(&SYSTEM);
        lazy_static::initialize(&COMPOSITOR);
        //COMPOSITOR.set_tracking_space(vr::TrackingUniverseOrigin::Standing);
        lh_install_hook(**glViewport as *mut _, glViewportHook as *mut _);
        lh_install_hook(**rw_camera_begin_update as *mut _, rw_camera_begin_update_hook as *mut _);
        lh_install_hook(**rw_camera_end_update as *mut _, rw_camera_end_update_hook as *mut _);
        lh_install_hook(**rw_camera_set_view_window as *mut _, rw_camera_set_view_window_hook as *mut _);
        let error = error_string();
        File::create("installed_hook.txt").unwrap();
        let mut errors = File::create("hook_errors.txt").unwrap();
        writeln!(&mut errors, "Error: {:?}", error);
        drop(errors);
    }
}

static counter: AtomicUsize = AtomicUsize::new(0);


pub extern "system" fn glViewportHook(x: i32, y: i32, width: u32, height: u32) {
    let current = counter.load(Ordering::SeqCst);
    counter.store(current.wrapping_add(1), Ordering::SeqCst);
    if current&2 == 0 {
        glViewport(x, y, width/2, height);
    } else {
        glViewport((width/2) as i32, y, width/2, height);
    }
}

fn camera_get_frame(camera: *mut c_void) -> *mut c_void {
    unsafe {
        let camera_as_ptrs = camera as *mut usize;
        let ptr_to_frame = camera_as_ptrs.offset(1);
        *ptr_to_frame as *mut c_void
    }
}

pub extern "C" fn rw_camera_begin_update_hook(camera: *mut c_void) -> *mut c_void {
    let current = counter.load(Ordering::SeqCst);
    if current&2 != 0 {
        let frame = camera_get_frame(camera);
        rw_frame_translate(frame, (&mut [-0.006, 0.0, 0.0]).as_mut_ptr(), 1);
    }
    //COMPOSITOR.wait_get_poses();
    rw_camera_begin_update(camera)
}

pub extern "C" fn rw_camera_end_update_hook(camera: *mut c_void) -> *mut c_void {
    
    let current = counter.load(Ordering::SeqCst);
    if current&2 == 0 {
        let mut texid = 0;
        unsafe {
            glGetIntegerv(0x8069, &mut texid);
        }
        if texid != 0 {
            let texture = vr::compositor::texture::Texture {
                handle: vr::compositor::texture::Handle::OpenGLTexture(texid as usize),
                color_space: vr::compositor::texture::ColorSpace::Auto
            };
            unsafe {
                let result = COMPOSITOR.submit(vr::Eye::Left, &texture, None, None);
                let mut errfile = std::fs::File::create("submit_error.txt").unwrap();
                writeln!(&mut errfile, "{:?}", result);
            }
        }
    }
    let result = rw_camera_end_update(camera);
    result
}

pub extern "C" fn rw_camera_set_view_window_hook(camera: *mut c_void, view_window: *mut f32) -> *mut c_void {
    if !view_window.is_null() {
        unsafe {
            *view_window = 1.0;
            *view_window.offset(1) = 1.0;
        }
    }
    unsafe {
        rw_camera_set_view_window(camera, view_window)
    }
}