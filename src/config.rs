/// How long to wait before ignoring the minimum delta fan threshold
/// Useful so we settle at actual min fan speeds at some point
pub const IGNORE_MIN_DELTA_THRESHOLD_AFTER_S: f32 = 13.0;

/// Polling rate
/// How often we update the temperature reading and potentially fan speed
pub const UPDATE_DELAY_S: f32 = 0.8;

/// Minimum delta fan speed, lower deltas than this won't trigger a fan speed change
pub const MIN_DELTA_FAN_THRESHOLD: f32 = 2.1;

/// Default sudo timeout, whould we not be able to read it from sudoers file
pub const SUDO_TIMESTAMP_TIMEOUT_DEFAULT_S: u64 = (5 * 60) - 10;

pub struct FanAtTemp {
    pub temp_c: f32,
    pub fan_pct: f32,
}

/// Actual fan curve
pub const CURVE: &'static [FanAtTemp] = &[
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
