package config

platform: {
	led_pin:                   uint8 & >=0 & <=28 | *25
	button_pin:                uint8 & >=0 & <=28 | *2
	control_period_ms:         uint32 & >0 | *1
	maintenance_period_ms:     uint32 & >0 | *100
	button_debounce_ms:        uint32 & >0 | *10

	// Display (SPI LCD) configuration
	display_refresh_ms:        uint32 & >0 | *33
	display_spi_freq_hz:       uint32 & >0 | *40000000
	display_width:             uint32 & >0 | *320
	display_height:            uint32 & >0 | *240

	// Bootloader configuration
	bootloader_staging_offset: uint32 & >0 | *1048576
	bootloader_staging_size:   uint32 & >0 | *1048576
	bootloader_chunk_size:     uint32 & >0 | *4096
	bootloader_uart_baud:      uint32 & >0 | *115200
}

#Platform: {
	pico1: platform & {
		led_pin:                   25
		button_pin:                2
		control_period_ms:         1
		maintenance_period_ms:     100
		button_debounce_ms:        10
		display_refresh_ms:        33
		display_spi_freq_hz:       40000000
		display_width:             320
		display_height:            240
		bootloader_staging_offset: 1048576   // 0x100000 (1MB into flash)
		bootloader_staging_size:   1048576   // 1MB staging
		bootloader_chunk_size:     4096
		bootloader_uart_baud:      115200
	}
	pico2: platform & {
		led_pin:                   25
		button_pin:                2
		control_period_ms:         1
		maintenance_period_ms:     100
		button_debounce_ms:        10
		display_refresh_ms:        33
		display_spi_freq_hz:       62500000
		display_width:             320
		display_height:            240
		bootloader_staging_offset: 2097152   // 0x200000 (2MB into flash)
		bootloader_staging_size:   2097152   // 2MB staging
		bootloader_chunk_size:     4096
		bootloader_uart_baud:      115200
	}
	unoq: platform & {
		led_pin:                   11 // PH11 (LED 3 Green, active-low)
		button_pin:                0  // No user button on UNO Q
		control_period_ms:         1
		maintenance_period_ms:     100
		button_debounce_ms:        10
		display_refresh_ms:        33
		display_spi_freq_hz:       40000000
		display_width:             320
		display_height:            240
		bootloader_staging_offset: 1048576   // 0x100000 (1MB into flash)
		bootloader_staging_size:   1048576   // 1MB staging
		bootloader_chunk_size:     4096      // STM32 8KB erase page, but 4KB write chunk
		bootloader_uart_baud:      115200
	}
}
