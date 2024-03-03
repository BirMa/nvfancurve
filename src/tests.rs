#[cfg(test)]
mod tests {
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
