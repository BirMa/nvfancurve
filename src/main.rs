// TODO - add mode where app bisects min fan pct values and uses those instead of hardcoded values
// TOOD - Kalman filter to get to target gpu temps? :D

mod config;
mod nv;
mod util;
mod x11;

use regex::Regex;
use std::{
    ffi::c_void,
    process::Command,
    ptr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{self, Duration, Instant},
};

const VERSION: &str = "0.0.1";

struct State {
    display: *mut *mut c_void,
    fan_cur: f32,
    fan_last_updated: Instant,
    fan_min_pct: f32,

    block_exit: Arc<AtomicBool>,
}

impl Default for State {
    fn default() -> Self {
        let block_exit = Arc::new(AtomicBool::new(false));
        Self {
            display: unsafe { x11::XOpenDisplay(ptr::null()) },
            fan_cur: 0.,
            fan_last_updated: Instant::now(),
            fan_min_pct: config::CURVE.first().unwrap().fan_pct,
            block_exit,
        }
    }
}

fn main() -> Result<(), String> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("Starting ({})...", VERSION);

    let mut state = State::default();
    let block_exit_handler_ref = state.block_exit.clone();
    let continue_looping = Arc::new(AtomicBool::new(true));
    let continue_looping_sigint_handler_ref = continue_looping.clone();
    let continue_looping_sudo_loop_ref = continue_looping.clone();

    ctrlc::set_handler(move || {
        log::info!("stopping...");
        let _ = nv::call_nv_settings_auto();
        if !block_exit_handler_ref.load(Ordering::SeqCst) {
            std::process::exit(0);
        }

        continue_looping_sigint_handler_ref.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    let sudo_timeout_s = get_sudo_timeout_s()?;
    log::debug!("sudo loop period: {}s", sudo_timeout_s);
    thread::spawn(move || {
        while continue_looping_sudo_loop_ref.load(Ordering::SeqCst) {
            thread::sleep(time::Duration::from_secs(sudo_timeout_s));
            log::debug!("sudo loop");
            if let Err(err) = util::call_sudo_nop() {
                log::warn!("sudo loop failed with '{}'", err);
                continue_looping_sudo_loop_ref.store(false, Ordering::SeqCst);
            }
        }
    });

    util::call_sudo_nop()?;

    // main loop
    loop {
        if !continue_looping.load(Ordering::SeqCst) {
            return Ok(());
        }

        thread::sleep(time::Duration::from_secs_f32(config::UPDATE_DELAY_S));

        let cur_temp = x11::get_nv_temp(0, state.display).unwrap() as f32;
        log::debug!("current nv temp: {:.0}C", cur_temp);
        log::debug!("current fan: {:.2}%", state.fan_cur);

        let desired_fan = tmp_to_fan(cur_temp, config::CURVE);
        state.fan_cur = set_fan(desired_fan, &mut state).unwrap();
    }
}

fn set_fan(desired_fan: f32, state: &mut State) -> Result<f32, String> {
    // So we settle at actual min fan speeds after not updating fans for a while
    let enough_time_passed = state.fan_last_updated.elapsed()
        > Duration::from_secs_f32(config::IGNORE_MIN_DELTA_THRESHOLD_AFTER_S);

    // To avoid overshooting at the slightest temp delta
    let enough_fan_delta = if enough_time_passed {
        (desired_fan - state.fan_cur).abs() > 0.05 // some hard coded tiny threshold so we stop setting the fan at some point
    } else {
        (desired_fan - state.fan_cur).abs() > config::MIN_DELTA_FAN_THRESHOLD
    };

    let fan_step_up = get_fan_step_up(state.fan_cur, desired_fan);
    // Bypass fan-step-up if we're just settling the fan at the desired value after not updating for a while
    // Causes fan zigzagging by 1 pct point, but that's (imho) the only way to keep stable temp
    // If only temps and fan speed weren integer...
    let new_fan = if enough_time_passed {
        desired_fan
    } else {
        (state.fan_cur + (desired_fan - state.fan_cur) * fan_step_up).max(state.fan_min_pct)
    };

    log::debug!(
        "fan delta: {:?} => {:?}, fan delta (with step-up): {:?}, time passed: {:?}",
        desired_fan - state.fan_cur,
        enough_fan_delta,
        new_fan - state.fan_cur,
        enough_time_passed
    );

    if !enough_fan_delta {
        log::debug!(
            "not changing fan (desired_fan: {:.2}, fan_cur: {:.2})",
            desired_fan,
            state.fan_cur
        );
        return Ok(state.fan_cur);
    }

    log::debug!(
        "trying to set fan to: \"{:.2}%\" ({:.2}% of the way to {:.2})",
        new_fan,
        fan_step_up * 100.,
        desired_fan
    );
    state.block_exit.store(true, Ordering::SeqCst);
    let result = nv::set_nv_fans(new_fan, state.fan_min_pct);
    state.block_exit.store(false, Ordering::SeqCst);

    state.fan_last_updated = Instant::now();

    result.map(|_| new_fan)
}

fn get_fan_step_up(cur_fan: f32, desired_fan: f32) -> f32 {
    const STEP_UP_CURVE: &'static [config::FanAtTemp] = &[
        config::FanAtTemp {
            temp_c: 1.,   // fan delta
            fan_pct: 0.2, // resulting step up pct
        },
        config::FanAtTemp {
            temp_c: 6.,
            fan_pct: 1.,
        },
    ];
    tmp_to_fan((cur_fan - desired_fan).abs(), STEP_UP_CURVE)
}

/* FAN CURVE */
fn tmp_to_fan(cur_temp: f32, curve: &[config::FanAtTemp]) -> f32 {
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
            config::SUDO_TIMESTAMP_TIMEOUT_DEFAULT_S
        },
        |c| {
            c.get(1).map_or_else(
                || {
                    log::debug!("(shouldn't happen) no timestamp_timeout match in output");
                    config::SUDO_TIMESTAMP_TIMEOUT_DEFAULT_S
                },
                |m| {
                    m.as_str().parse().unwrap_or_else(|x| {
                        log::debug!("failed parsing timestamp_timeout: {}", x);
                        config::SUDO_TIMESTAMP_TIMEOUT_DEFAULT_S
                    })
                },
            )
        },
    );

    let timeout_s = timeout_m * 60 - 5;

    Ok(if timeout_s <= 0 {
        log::debug!(
            "sudo timeout too low, setting to default {}s",
            config::SUDO_TIMESTAMP_TIMEOUT_DEFAULT_S
        );
        config::SUDO_TIMESTAMP_TIMEOUT_DEFAULT_S
    } else {
        timeout_s
    })
}
