package config

platform: {
	led_pin:                   uint8 & >=0 & <=28 | *25
	button_pin:                uint8 & >=0 & <=28 | *2
	control_period_ms:         uint32 & >0 | *1
	maintenance_period_ms:     uint32 & >0 | *100
	button_debounce_ms:        uint32 & >0 | *10
	button_release_delay_ms:   uint32 & >0 | *50
	control_log_interval:      uint32 & >0 | *1000
}

#Platform: {
	pico1: platform & {
		led_pin:                   25
		button_pin:                2
		control_period_ms:         1
		maintenance_period_ms:     100
		button_debounce_ms:        10
		button_release_delay_ms:   50
		control_log_interval:      1000
	}
	pico2: platform & {
		led_pin:                   25
		button_pin:                2
		control_period_ms:         1
		maintenance_period_ms:     100
		button_debounce_ms:        10
		button_release_delay_ms:   50
		control_log_interval:      1000
	}
}

selected: #Platform.pico1
