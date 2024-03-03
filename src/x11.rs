use std::ffi::{c_char, c_int, c_uint, c_void};

const XNV_OK: i32 = 1;

type Display = *mut c_void;

pub fn get_nv_temp(id: u32, display: *mut *mut c_void) -> Result<i32, String> {
    let mut tmp = -1i32;

    match unsafe {
        XNVCTRLQueryTargetAttribute(
            display,
            CTRL_TARGET::GPU,
            id as i32,
            0,
            CTRL_ATTR::CORE_TEMPERATURE,
            &mut tmp,
        )
    } {
        XNV_OK => Ok(tmp),
        i => Err(format!(
            "XNVCtrl QueryAttr(CORE_TEMPERATURE) failed; error {}",
            i
        )),
    }
}

// FFI related stuff

/// XNVCtrl target
#[allow(dead_code)]
#[allow(non_camel_case_types)]
#[repr(u32)]
enum CTRL_TARGET {
    X_SCREEN = 0,
    GPU = 1,
    FRAMELOCK = 2,
    VCSC = 3,
    GVI = 4,
    COOLER = 5,
    THERMAL_SENSOR = 6,
    _3D_VISION_PRO_TRANSCEIVER = 7,
    DISPLAY = 8,
}

/// XNVCtrl Attribute (non exhaustive)
#[allow(dead_code)]
#[allow(non_camel_case_types)]
#[repr(u32)]
enum CTRL_ATTR {
    NVIDIA_DRIVER_VERSION = 3,
    PRODUCT_NAME = 0,
    UTILIZATION = 53,
    CORE_TEMPERATURE = 60,
    CORE_THRESHOLD = 61,
    DEFAULT_CORE_THRESHOLD = 62,
    MAX_CORE_THRESHOLD = 63,
    COOLER_MANUAL_CONTROL = 319,
    THERMAL_COOLER_LEVEL = 320,
    THERMAL_COOLER_SPEED = 405,
    THERMAL_COOLER_CURRENT_LEVEL = 417,
}

#[allow(dead_code)]
#[link(name = "X11")]
#[link(name = "Xext")]
#[link(name = "XNVCtrl")]
extern "C" {
    //https://github.com/foucault/nvfancontrol/blob/547dab69775fe7cd4ec7e9d91d28d549dcc9e13f/src/nvctrl/os/unix.rs#L74

    /// Opens a new X11 display with the specified name
    ///
    /// **Arguments**
    ///
    /// * `name` - Name of the display to open
    pub fn XOpenDisplay(name: *const c_char) -> *mut Display;

    /// XNVCtrl int query with target
    ///
    /// **Arguments**
    ///
    /// * `dpy` - The current X11 `Display`
    /// * `target` - Attribute query target (`CTRL_TARGET`)
    /// * `id` - GPU id
    /// * `mask` - Attribute mask
    /// * `attribute` - Attribute to query (`CTRL_ATTR`)
    /// * `value` - The value of the attribute that will be populated upon function call
    fn XNVCTRLQueryTargetAttribute(
        dpy: *const Display,
        target: CTRL_TARGET,
        id: c_int,
        mask: c_uint,
        attribute: CTRL_ATTR,
        value: *mut c_int,
    ) -> c_int;
}
