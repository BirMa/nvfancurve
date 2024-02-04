use regex::Regex;
use std::{
    ffi::{c_char, c_int, c_uint, c_void},
    process::Command,
    ptr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread, time,
};

struct FanAtTemp {
    temp_c: f32,
    fan_pct: f32,
}

// config
const UPDATE_DELAY_S: f32 = 0.8;
const MIN_DELTA_FAN_THRESHOLD: f32 = 2.1;
const SUDO_TIMESTAMP_TIMEOUT_DEFAULT_S: u64 = (5 * 60) - 10;
const CURVE: &'static [FanAtTemp] = &[
    FanAtTemp {
        temp_c: 40.,
        fan_pct: 43.,
    },
    FanAtTemp {
        temp_c: 63.,
        fan_pct: 60.,
    },
    FanAtTemp {
        temp_c: 75.,
        fan_pct: 83.,
    },
    FanAtTemp {
        temp_c: 81.,
        fan_pct: 100.,
    },
];

fn main() -> Result<(), String> {
    // setup
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let block_exit = Arc::new(AtomicBool::new(false));
    let block_exit_clone = block_exit.clone();

    let continue_looping = Arc::new(AtomicBool::new(true));
    let continue_looping_clone_sigint_handler = continue_looping.clone();
    let continue_looping_clone_sudo_loop = continue_looping.clone();

    ctrlc::set_handler(move || {
        log::info!("stopping...");
        let _ = call_nv_settings_off();
        if !block_exit_clone.load(Ordering::SeqCst) {
            std::process::exit(0);
        }
        continue_looping_clone_sigint_handler.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    let sudo_timeout_s = get_sudo_timeout_s()?;
    log::debug!("sudo loop period: {}s", sudo_timeout_s);
    thread::spawn(move || {
        while continue_looping_clone_sudo_loop.load(Ordering::SeqCst) {
            thread::sleep(time::Duration::from_secs(sudo_timeout_s));
            log::debug!("sudo loop");
            if let Err(err) = call_sudo_nop() {
                log::warn!("sudo loop failed with '{}'", err);
                continue_looping_clone_sudo_loop.store(false, Ordering::SeqCst);
            }
        }
    });

    call_sudo_nop()?;

    let mut cur_fan = 0.;
    let display = unsafe { XOpenDisplay(ptr::null()) };

    // main loop
    loop {
        if !continue_looping.load(Ordering::SeqCst) {
            return Ok(());
        }

        thread::sleep(time::Duration::from_secs_f32(UPDATE_DELAY_S));

        let cur_temp = get_nv_temp(0, display).unwrap() as f32;
        log::debug!("current nv temp: {:.0}C", cur_temp);
        log::debug!("current fan: {:.2}%", cur_fan);
        let desired_fan = tmp_to_fan(cur_temp, CURVE);

        cur_fan = set_fan(
            desired_fan,
            cur_fan,
            CURVE.first().unwrap().fan_pct,
            get_fan_step_up(cur_fan, desired_fan),
            &block_exit,
        )
        .unwrap();
    }
}

fn set_fan(
    desired_fan: f32,
    cur_fan: f32,
    min_fan_pct: f32,
    fan_step_up: f32,
    block_exit: &Arc<AtomicBool>,
) -> Result<f32, String> {
    if (desired_fan - cur_fan).abs() <= MIN_DELTA_FAN_THRESHOLD {
        log::debug!(
            "not changing fan (desired_fan: {:.2}, cur_fan: {:.2})",
            desired_fan,
            cur_fan
        );
        return Ok(cur_fan);
    }

    let new_fan = (cur_fan + (desired_fan - cur_fan) * fan_step_up).max(min_fan_pct);

    if (new_fan - cur_fan).abs() <= MIN_DELTA_FAN_THRESHOLD {
        log::debug!(
            "not changing fan (new_fan: {:.2}, cur_fan: {:.2})",
            new_fan,
            cur_fan
        );
        return Ok(cur_fan);
    }

    log::debug!(
        "trying to set fan to: \"{:.2}%\" ({:.2}% of the way to {:.2})",
        new_fan,
        fan_step_up * 100.,
        desired_fan
    );
    block_exit.store(true, Ordering::SeqCst);
    let result = set_nv_fans(new_fan, min_fan_pct);
    block_exit.store(false, Ordering::SeqCst);
    result.map(|_| new_fan)
}

fn get_fan_step_up(cur_fan: f32, desired_fan: f32) -> f32 {
    const STEP_UP_CURVE: &'static [FanAtTemp] = &[
        FanAtTemp {
            temp_c: 1.,   // fan delta
            fan_pct: 0.2, // resulting step up pct
        },
        FanAtTemp {
            temp_c: 6.,
            fan_pct: 1.,
        },
    ];
    tmp_to_fan((cur_fan - desired_fan).abs(), STEP_UP_CURVE)
}

/* FAN CURVE */
fn tmp_to_fan(cur_temp: f32, curve: &[FanAtTemp]) -> f32 {
    if cur_temp <= curve.first().unwrap().temp_c {
        return curve.first().unwrap().fan_pct;
    }

    if cur_temp >= curve.last().unwrap().temp_c {
        return curve.last().unwrap().fan_pct;
    }

    for (idx, e) in curve.iter().enumerate() {
        let min_t = e.temp_c;
        let min_f = e.fan_pct;
        let max_t = curve[idx + 1].temp_c;
        let max_f = curve[idx + 1].fan_pct;

        if min_t <= cur_temp && cur_temp <= max_t {
            let slope = (max_f - min_f) / (max_t - min_t);
            let offset_at_x0 = min_f - slope * min_t;
            let fan_out = slope * cur_temp + offset_at_x0;
            return fan_out;
        }
    }

    log::warn!("fan curve not hit, returning max fan");
    100.
}

#[cfg(test)]
mod tests {
    use crate::tmp_to_fan;
    use crate::FanAtTemp;

    #[test]
    fn test() {
        const TEST_CURVE: &'static [FanAtTemp] = &[
            FanAtTemp {
                temp_c: 40.,
                fan_pct: 43.,
            },
            FanAtTemp {
                temp_c: 65.,
                fan_pct: 60.,
            },
            FanAtTemp {
                temp_c: 78.,
                fan_pct: 80.,
            },
            FanAtTemp {
                temp_c: 84.,
                fan_pct: 100.,
            },
        ];

        test_with(f32::NEG_INFINITY, 43., TEST_CURVE);
        test_with(20., 43., TEST_CURVE);

        test_with(40., 43., TEST_CURVE);
        test_with(41., 43.68, TEST_CURVE);

        test_with(52.5, 51.5, TEST_CURVE);

        test_with(64.9999, 59.99993, TEST_CURVE);
        test_with(65., 60., TEST_CURVE);
        test_with(65.0001, 60.000153, TEST_CURVE);

        test_with(71.5, 70., TEST_CURVE);

        test_with(77.9999, 79.99985, TEST_CURVE);
        test_with(78., 80., TEST_CURVE);
        test_with(78.0001, 80.000336, TEST_CURVE);

        test_with(81., 90., TEST_CURVE);

        test_with(83.9999, 99.999664, TEST_CURVE);
        test_with(84., 100., TEST_CURVE);

        test_with(85., 100., TEST_CURVE);
        test_with(1000., 100., TEST_CURVE);
    }

    fn test_with(input_temp: f32, expected_fan: f32, curve: &[FanAtTemp]) {
        let actual_fan = tmp_to_fan(input_temp, curve);
        assert_eq!(
            expected_fan,
            tmp_to_fan(input_temp, curve),
            "got {} for input temp {}, expected {}",
            actual_fan,
            input_temp,
            expected_fan
        );
    }
}

/* NV STUFF */
fn set_nv_fans(fan: f32, fan_min: f32) -> Result<(), String> {
    call_xhost_add()?;

    let fan0 = (fan.ceil() as i8).min(100);
    let fan1 = (fan.floor() as i8).max(fan_min as i8);

    log::info!("setting fans to ({:}%, {:}%)", fan0, fan1);
    if let Err(mut err) = call_nv_settings(fan0, fan1) {
        if let Err(err_inner) = call_xhost_remove() {
            err = format!(
                "{} happened, but during cleanup {} happened as well!",
                err, err_inner
            );
        }
        return Err(err);
    };

    call_xhost_remove()
}

/// calls xhost si:localuser:root
fn call_xhost_add() -> Result<(), String> {
    make_call("xhost add root", "xhost", &["-si:localuser:root"].to_vec())
}

/// xhost -si:localuser:root
fn call_xhost_remove() -> Result<(), String> {
    make_call(
        "xhost remove root",
        "xhost",
        &["-si:localuser:root"].to_vec(),
    )
}

/// sudo nvidia-settings -a "*:1[gpu:0]/GPUFanControlState=1" -a "*:1[fan-0]/GPUTargetFanSpeed=$PCT" -a "*:1[fan-1]/GPUTargetFanSpeed=$PCT"
fn call_nv_settings(fan_speed0: i8, fan_speed1: i8) -> Result<(), String> {
    make_call(
        "nvidia-settings",
        "sudo",
        &[
            "nvidia-settings",
            "-a",
            "*:1[gpu:0]/GPUFanControlState=1",
            "-a",
            &format!("*:1[fan-0]/GPUTargetFanSpeed={}", fan_speed0),
            "-a",
            &format!("*:1[fan-1]/GPUTargetFanSpeed={}", fan_speed1),
        ]
        .to_vec(),
    )
}

/// sudo nvidia-settings -a "*:1[gpu:0]/GPUFanControlState=0"
fn call_nv_settings_off() -> Result<(), String> {
    make_call(
        "nvidia-settings",
        "sudo",
        &["nvidia-settings", "-a", "*:1[gpu:0]/GPUFanControlState=0"].to_vec(),
    )
}

fn call_sudo_nop() -> Result<(), String> {
    make_call("sudo loop", "sudo", &["true"].to_vec())
}

fn make_call(name: &str, prog: &str, args: &Vec<&str>) -> Result<(), String> {
    let output = match Command::new(prog).args(args).output() {
        Ok(output) => output,
        Err(err) => return Err(format!("command {} failed: {}", name, err)),
    };
    log_output(output.stdout);
    Ok(())
}

fn log_output(output: Vec<u8>) {
    log::trace!(
        "\"\"\"{}\"\"\"",
        std::str::from_utf8(&output)
            .or::<String>(Ok("<could not read output as utf-8>"))
            .unwrap()
    );
}

/* X11 STUFF */
fn get_nv_temp(id: u32, display: *mut *mut c_void) -> Result<i32, String> {
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

const XNV_OK: i32 = 1;

type Display = *mut c_void;
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
    fn XOpenDisplay(name: *const c_char) -> *mut Display;

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

// OS stuff
fn get_sudo_timeout_s() -> Result<u64, String> {
    let re = Regex::new(r"timestamp_timeout=(?<t>\d+)").unwrap();

    // sudo -l to get defaults of current user
    let output = Command::new("sudo")
        .args(["-l"])
        .output()
        .or_else(|x| Err(format!("error calling sudo {}", x)))?;
    let out = String::from_utf8_lossy(&output.stdout);
    let captures = re.captures_iter(&out).last();

    let timeout_m = captures.map_or_else(
        || {
            log::debug!("no timestamp_timeout match in output");
            SUDO_TIMESTAMP_TIMEOUT_DEFAULT_S
        },
        |c| {
            c.get(1).map_or_else(
                || {
                    log::debug!("(shouldn't happen) no timestamp_timeout match in output");
                    SUDO_TIMESTAMP_TIMEOUT_DEFAULT_S
                },
                |m| {
                    m.as_str().parse().unwrap_or_else(|x| {
                        log::debug!("failed parsing timestamp_timeout: {}", x);
                        SUDO_TIMESTAMP_TIMEOUT_DEFAULT_S
                    })
                },
            )
        },
    );

    let timeout_s = timeout_m * 60 - 5;

    Ok(if timeout_s <= 0 {
        log::debug!(
            "sudo timeout too low, setting to default {}s",
            SUDO_TIMESTAMP_TIMEOUT_DEFAULT_S
        );
        SUDO_TIMESTAMP_TIMEOUT_DEFAULT_S
    } else {
        timeout_s
    })
}
