package config

platform: {
	led_pin:                   uint8 & >=0 & <=28 | *25
	button_pin:                uint8 & >=0 & <=28 | *2
	control_period_ms:         uint32 & >0 | *1
	maintenance_period_ms:     uint32 & >0 | *100
	button_debounce_ms:        uint32 & >0 | *10
	button_release_delay_ms:   uint32 & >0 | *50
	control_log_interval:      uint32 & >0 | *1000

	// Display (SPI LCD) configuration
	display_refresh_ms:        uint32 & >0 | *33
	display_spi_freq_hz:       uint32 & >0 | *40000000
	display_width:             uint32 & >0 | *320
	display_height:            uint32 & >0 | *240
	display_sck_pin:           uint8 | *18
	display_mosi_pin:          uint8 | *19
	display_cs_pin:            uint8 | *17
	display_dc_pin:            uint8 | *20
	display_rst_pin:           uint8 | *21
	display_bl_pin:            uint8 | *22
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
		display_refresh_ms:        33
		display_spi_freq_hz:       40000000
		display_width:             320
		display_height:            240
		display_sck_pin:           18
		display_mosi_pin:          19
		display_cs_pin:            17
		display_dc_pin:            20
		display_rst_pin:           21
		display_bl_pin:            22
	}
	pico2: platform & {
		led_pin:                   25
		button_pin:                2
		control_period_ms:         1
		maintenance_period_ms:     100
		button_debounce_ms:        10
		button_release_delay_ms:   50
		control_log_interval:      1000
		display_refresh_ms:        33
		display_spi_freq_hz:       62500000
		display_width:             320
		display_height:            240
		display_sck_pin:           18
		display_mosi_pin:          19
		display_cs_pin:            17
		display_dc_pin:            20
		display_rst_pin:           21
		display_bl_pin:            22
	}
	unoq: platform & {
		led_pin:                   11 // PH11 (LED 3 Green, active-low)
		button_pin:                0  // No user button on UNO Q
		control_period_ms:         1
		maintenance_period_ms:     100
		button_debounce_ms:        10
		button_release_delay_ms:   50
		control_log_interval:      1000
		display_refresh_ms:        33
		display_spi_freq_hz:       40000000
		display_width:             320
		display_height:            240
		display_sck_pin:           5   // PA5 (SPI1_SCK)
		display_mosi_pin:          7   // PA7 (SPI1_MOSI)
		display_cs_pin:            6   // PA6 (SPI1_NSS)
		display_dc_pin:            4   // PA4
		display_rst_pin:           3   // PA3
		display_bl_pin:            2   // PA2
	}
}

selected: #Platform.unoq
