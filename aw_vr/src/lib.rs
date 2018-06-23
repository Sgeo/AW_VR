#![crate_type="cdylib"]

extern crate easyhook;
extern crate libloading as lib;
extern crate ovr_sys as vr;
extern crate nalgebra;

#[macro_use]
extern crate lazy_static;

use std::io::Write;
use std::os::raw::{c_ulong, c_void};
use std::sync::atomic::{AtomicUsize, Ordering};
use easyhook::{lh_install_hook};
use easyhook::error_string;
use std::sync::Mutex;
use std::mem;

#[derive(Debug)]
struct Session(vr::ovrSession);

unsafe impl Sync for Session {}

impl std::ops::Deref for Session {
    type Target = vr::ovrSession;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Session {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug)]
struct TextureSwapChain(vr::ovrTextureSwapChain);

unsafe impl Send for TextureSwapChain {}
unsafe impl Sync for TextureSwapChain {}

impl std::ops::Deref for TextureSwapChain {
    type Target = vr::ovrTextureSwapChain;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for TextureSwapChain {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Drop for TextureSwapChain {
    fn drop(&mut self) {
        unsafe {
            vr::ovr_DestroyTextureSwapChain(**VRSession, self.0);
        }
    }
}

lazy_static! {
    static ref GL: lib::Library = lib::Library::new("OPENGL32").unwrap();
    static ref RW: lib::Library = lib::Library::new("rw_opengl").unwrap();
    static ref glViewport: lib::Symbol<'static, extern "system" fn(i32, i32, u32, u32)> = unsafe { GL.get(b"glViewport\0") }.unwrap();
    static ref glGetIntegerv: lib::Symbol<'static, extern "system" fn(i32, *mut i32)> = unsafe { GL.get(b"glGetIntegerv\0") }.unwrap();
    static ref glBindTexture: lib::Symbol<'static, extern "system" fn(i32, u32)> = unsafe { GL.get(b"glBindTexture\0") }.unwrap();
    static ref glReadBuffer: lib::Symbol<'static, extern "system" fn(i32)> = unsafe { GL.get(b"glReadBuffer\0") }.unwrap();
    static ref glCopyPixels: lib::Symbol<'static, extern "system" fn(i32, i32, u32, u32, i32)> = unsafe { GL.get(b"glCopyPixels\0") }.unwrap();
    static ref glCopyTexImage2D: lib::Symbol<'static, extern "system" fn(i32, i32, i32, i32, i32, u32, u32, i32)> = unsafe { GL.get(b"glCopyTexImage2D\0") }.unwrap();
    static ref glCopyTexSubImage2D: lib::Symbol<'static, extern "system" fn(i32, i32, i32, i32, i32, i32, u32, u32)> = unsafe { GL.get(b"glCopyTexSubImage2D\0") }.unwrap();
    static ref glEnable: lib::Symbol<'static, extern "system" fn(i32)> = unsafe { GL.get(b"glEnable\0") }.unwrap();
    static ref glGetError: lib::Symbol<'static, extern "system" fn() -> i32> = unsafe { GL.get(b"glGetError\0") }.unwrap();
    static ref rw_camera_begin_update: lib::Symbol<'static, extern "C" fn(*mut c_void) -> *mut c_void> = unsafe { RW.get(b"rw_camera_begin_update\0") }.unwrap();
    static ref rw_camera_end_update: lib::Symbol<'static, extern "C" fn(*mut c_void) -> *mut c_void> = unsafe { RW.get(b"rw_camera_end_update\0") }.unwrap();
    static ref rw_frame_translate: lib::Symbol<'static, extern "C" fn(*mut c_void, *mut f32, u32) -> *mut c_void> = unsafe { RW.get(b"rw_frame_translate\0") }.unwrap();
    static ref rw_frame_rotate: lib::Symbol<'static, extern "C" fn(*mut c_void, *mut f32, f32, u32) -> *mut c_void> = unsafe { RW.get(b"rw_frame_rotate\0") }.unwrap();
    static ref rw_camera_set_view_window: lib::Symbol<'static, extern "C" fn(*mut c_void, *mut f32) -> *mut c_void> = unsafe { RW.get(b"rw_camera_set_view_window\0") }.unwrap();
    static ref rw_camera_resize: lib::Symbol<'static, extern "C" fn(*mut c_void, i32, i32) -> *mut c_void> = unsafe { RW.get(b"rw_camera_resize\0") }.unwrap();
}

lazy_static! {
    static ref VRSession: Session = {
        unsafe {
            let init = vr::ovrInitParams {
                Flags: 0,
                RequestedMinorVersion: vr::OVR_MINOR_VERSION,
                LogCallback: None,
                UserData: 0,
                ConnectionTimeoutMS: 0,
                .. mem::uninitialized()
            };
            let result = vr::ovr_Initialize(&init as *const _);
            let mut session: vr::ovrSession = mem::uninitialized();
            let mut luid: vr::ovrGraphicsLuid = mem::uninitialized();
            let result = vr::ovr_Create(&mut session as *mut _, &mut luid as *mut _);
            Session(session)
        }
    };
    static ref VRTextureSwapChains: Mutex<Option<[TextureSwapChain; 2]>> = Mutex::new(None);
    static ref ViewportSize: Mutex<Option<(u32, u32)>> = Mutex::new(None);
    static ref VRPoses: Mutex<[vr::ovrPosef; 2]> = Mutex::new([zero_posef(), zero_posef()]);
}

fn zero_posef() -> vr::ovrPosef {
    unsafe {
        vr::ovrPosef {
            Position: vr::ovrVector3f {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                .. mem::uninitialized()
            },
            Orientation: vr::ovrQuatf {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
                .. mem::uninitialized()
            },
            .. mem::uninitialized()
        }
    }
}

fn texture_swap_chain(width: i32, height: i32) -> TextureSwapChain {
    let desc = vr::ovrTextureSwapChainDesc {
        Type: vr::ovrTexture_2D,
        Format: vr::OVR_FORMAT_R8G8B8A8_UNORM_SRGB,
        ArraySize: 1,
        Width: width,
        Height: height,
        MipLevels: 1,
        SampleCount: 1,
        StaticImage: 0,
        MiscFlags: 0,
        BindFlags: 0
    };
    unsafe {
        let mut tsc = mem::uninitialized();
        vr::opengl::ovr_CreateTextureSwapChainGL(**VRSession, &desc, &mut tsc);
        TextureSwapChain(tsc)
    }
}

#[export_name="_NativeInjectionEntryPoint_4"] // EasyHook32.dll has been hex edited to look for this
pub extern "stdcall" fn NativeInjectionEntryPoint(_remote_info: *mut c_void) {
    unsafe {
        use std::fs::File;
        use std::mem;
        File::create("about_to_install_hook.txt").unwrap();

        lazy_static::initialize(&VRSession);
        vr::ovr_SetTrackingOriginType(**VRSession, vr::ovrTrackingOrigin_FloorLevel);
        
        //lh_install_hook(**glViewport as *mut _, glViewportHook as *mut _);
        lh_install_hook(**rw_camera_begin_update as *mut _, rw_camera_begin_update_hook as *mut _);
        lh_install_hook(**rw_camera_end_update as *mut _, rw_camera_end_update_hook as *mut _);
        lh_install_hook(**rw_camera_set_view_window as *mut _, rw_camera_set_view_window_hook as *mut _);
        lh_install_hook(**rw_camera_resize as *mut _, rw_camera_resize_hook as *mut _);
        let error = error_string();
        File::create("installed_hook.txt").unwrap();
        let mut errors = File::create("hook_errors.txt").unwrap();
        writeln!(&mut errors, "Error: {:?}", error);
        drop(errors);
        
    }
}

static counter: AtomicUsize = AtomicUsize::new(0);


pub extern "system" fn glViewportHook(x: i32, y: i32, width: u32, height: u32) {
    *ViewportSize.lock().unwrap() = Some((width, height));
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
    if current&1 == 0 {
        unsafe {
            let fov = vr::ovrFovPort {
                UpTan: 1.0,
                DownTan: 1.0,
                LeftTan: 1.0,
                RightTan: 1.0,
                .. mem::uninitialized()
            };
            let left_eye_hmd_offset = vr::ovr_GetRenderDesc(**VRSession, vr::ovrEye_Left, fov).HmdToEyeOffset;
            let right_eye_hmd_offset = vr::ovr_GetRenderDesc(**VRSession, vr::ovrEye_Right, fov).HmdToEyeOffset;
            let mut poses = [zero_posef(), zero_posef()];
            vr::ovr_GetEyePoses(**VRSession, 0, 1, &[left_eye_hmd_offset, right_eye_hmd_offset], (&mut poses).as_mut_ptr() as *const _, std::ptr::null_mut());
            scale_posef(&mut poses[0]);
            scale_posef(&mut poses[1]);
            *VRPoses.lock().unwrap() = poses;
        }
    }
    let eye = current&1;
    let frame = camera_get_frame(camera);
    let eye_pose = VRPoses.lock().unwrap()[eye];
    rw_frame_translate(frame, (&mut [eye_pose.Position.x, eye_pose.Position.y, eye_pose.Position.z]).as_mut_ptr(), 1);
    let quat = nalgebra::geometry::Quaternion::new(eye_pose.Orientation.w, eye_pose.Orientation.x, eye_pose.Orientation.y, eye_pose.Orientation.z);
    let (_norm, angle, maybe_axis) = quat.polar_decomposition();
    let axis = maybe_axis.map(|axis| {
        let axis = axis.as_ref();
        (axis[0], axis[1], axis[2])
    }).unwrap_or((1.0, 0.0, 0.0));
    rw_frame_rotate(frame, (&mut [-axis.0, axis.1, -axis.2]).as_mut_ptr(), 2.0 * angle.to_degrees(), 1);
    let result = rw_camera_begin_update(camera);
    result
}

fn scale_posef(pose: &mut vr::ovrPosef) {
    pose.Position.x /= -10.0;
    pose.Position.y /= 10.0;
    pose.Position.z /= -10.0;
}

fn layer(tsc: &[TextureSwapChain], viewport_size: (u32, u32), poses: &[vr::ovrPosef]) -> vr::ovrLayerEyeFov {
    let (width, height) = viewport_size;
    unsafe { 
        let viewport = vr::ovrRecti {
            Pos: vr::ovrVector2i {
                x: 0,
                y: 0,
                .. mem::uninitialized()
            },
            Size: vr::ovrSizei {
                w: width as i32,
                h: height as i32,
                .. mem::uninitialized()
            },
            .. mem::uninitialized()
        };
        let fov = vr::ovrFovPort {
            UpTan: 1.0,
            DownTan: 1.0,
            LeftTan: 1.0,
            RightTan: 1.0,
            .. mem::uninitialized()
        };
        vr::ovrLayerEyeFov {
            Header: vr::ovrLayerHeader {
                Type: vr::ovrLayerType_EyeFov,
                Flags: vr::ovrLayerFlag_TextureOriginAtBottomLeft as u32,
                .. mem::uninitialized()
            },
            ColorTexture: [*tsc[0], *tsc[1]],
            Viewport: [viewport, viewport],
            Fov: [fov, fov],
            RenderPose: [poses[0], poses[1]],
            SensorSampleTime: 0.0,
            .. mem::uninitialized()
        }
    }
}

fn check_error<S: AsRef<str>>(where_: S) {
    let error = glGetError();
    if error != 0 {
        use std::fs::File;
        
        let mut file = File::create("vrerror.txt").unwrap();
        write!(&mut file, "GL Error! Error code: 0x{:X} after doing: {}", error, where_.as_ref()).unwrap();
        panic!("GL ERROR");
    }
}

pub extern "C" fn rw_camera_end_update_hook(camera: *mut c_void) -> *mut c_void {
    let result = rw_camera_end_update(camera);
    let current = counter.load(Ordering::SeqCst);
    let eye: usize = current&1;
    unsafe {
        let mut texid = 0;
        let mut tsc_lock = VRTextureSwapChains.lock().unwrap();
        if tsc_lock.is_none() {
            let mut viewport = [0i32, 0, 0, 0];
            glGetIntegerv(0x0BA2, viewport.as_mut_ptr());
            let (width, height) = (viewport[2] as u32, viewport[3] as u32);
            //let (width, height) = (512, 512);
            *tsc_lock = Some([texture_swap_chain(width as i32, height as i32), texture_swap_chain(width as i32, height as i32)]);
            *ViewportSize.lock().unwrap() = Some((width, height));
        }
        let tsc = tsc_lock.as_ref().unwrap();
        vr::opengl::ovr_GetTextureSwapChainBufferGL(**VRSession, *tsc[eye], -1, &mut texid);
        if texid == 0 {
            panic!("0 texid");
        }
        let (width, height) = ViewportSize.lock().unwrap().unwrap();
        
        glEnable(0x0DE1);
        check_error("Enabling GL_TEXTURE_2D");
        glReadBuffer(0x0404);
        check_error("glReadBuffer");
        glBindTexture(0x0DE1, texid);
        check_error("glBindTexture");
        glCopyTexSubImage2D(0x0DE1, 0, 0, 0, 0, 0, width, height);
        check_error("glCopyTexSubImage2D");
        vr::ovr_CommitTextureSwapChain(**VRSession, *tsc[eye]);
        if eye == 1 {
            let poses = VRPoses.lock().unwrap();
            let layer = layer(&*tsc, (width, height), &*poses);
            let layers = [&layer as *const _ as *const vr::ovrLayerHeader];
            vr::ovr_SubmitFrame(**VRSession, 0, std::ptr::null(), (&layers).as_ptr(), 1);
        }
    }
    counter.store(current.wrapping_add(1), Ordering::SeqCst);
    result
}

pub extern "C" fn rw_camera_set_view_window_hook(camera: *mut c_void, view_window: *mut f32) -> *mut c_void {
    if !view_window.is_null() {
        unsafe {
            //*view_window /= 2.0;
            *view_window = 1.0;
            *view_window.offset(1) = 1.0;
        }
    }
    unsafe {
        rw_camera_set_view_window(camera, view_window)
    }
}

pub extern "C" fn rw_camera_resize_hook(camera: *mut c_void, width: i32, height: i32) -> *mut c_void {
    *VRTextureSwapChains.lock().unwrap() = Some([texture_swap_chain(width, height), texture_swap_chain(width, height)]);
    *ViewportSize.lock().unwrap() = Some((width as u32, height as u32));
    camera
}