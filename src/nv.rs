use crate::util;

pub fn set_nv_fans(fan: f32, fan_min: f32) -> Result<(), String> {
    call_xhost_add()?;

    // "Smooth" the int van pct value out across the two available fan integer speeds
    let low = 0.2;
    let high = 0.7;

    let ceil = (fan.ceil() as i8).min(100);
    let floor = (fan.floor() as i8).max(fan_min as i8);
    let rem = fan % 1.0;
    let fan_low = if rem > high { ceil } else { floor };
    let fan_high = if rem > low { ceil } else { floor };

    log::debug!("\"rounding\" {:?} to {:?} and {:?}", fan, fan_low, fan_high);

    log::info!("setting fans to ({:}%, {:}%)", fan_low, fan_high);
    if let Err(mut err) = call_nv_settings(fan_low, fan_high) {
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

/// sudo nvidia-settings -a "*:1[gpu:0]/GPUFanControlState=1" -a "*:1[fan-0]/GPUTargetFanSpeed=$PCT" -a "*:1[fan-1]/GPUTargetFanSpeed=$PCT"
pub fn call_nv_settings(fan_speed0: i8, fan_speed1: i8) -> Result<(), String> {
    util::make_call(
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
pub fn call_nv_settings_off() -> Result<(), String> {
    util::make_call(
        "nvidia-settings",
        "sudo",
        &["nvidia-settings", "-a", "*:1[gpu:0]/GPUFanControlState=0"].to_vec(),
    )
}

/// calls xhost si:localuser:root
fn call_xhost_add() -> Result<(), String> {
    util::make_call("xhost add root", "xhost", &["-si:localuser:root"].to_vec())
}

/// xhost -si:localuser:root
fn call_xhost_remove() -> Result<(), String> {
    util::make_call(
        "xhost remove root",
        "xhost",
        &["-si:localuser:root"].to_vec(),
    )
}
