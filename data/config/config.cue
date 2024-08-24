package config

// CUE owns *hardware* configuration only (pins).
// Actor timing (periods, jitter, deadlines, priority, core) lives in SysML
// (`model/*.sysml`) and is code-generated on each actor struct.
platform: {
	led_pin:    uint8 & >=0 & <=28 | *25
	button_pin: uint8 & >=0 & <=28 | *2
}

#Platform: {
	pico1: platform & {
		led_pin:    25
		button_pin: 2
	}
	pico2: platform & {
		led_pin:    25
		button_pin: 2
	}
	unoq: platform & {
		// On-board STM32 peripherals (see Zephyr arduino_uno_q-common.dtsi)
		led_pin:    11 // PH11 = RGB LED 3 green (GPIO_ACTIVE_LOW)
		// No on-board user button (only power button). D7/PB2 = Arduino header pin 7.
		button_pin: 7
	}
}
