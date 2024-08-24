/* STM32U585 Memory Layout
 * Reference: STM32U585AIIx datasheet
 * - Flash: 2MB (0x0800_0000 - 0x081F_FFFF)
 * - SRAM1: 192KB (0x2000_0000 - 0x2002_FFFF)
 * - SRAM2: 64KB (0x2003_0000 - 0x2003_FFFF)
 * - SRAM3: 512KB (0x2004_0000 - 0x200B_FFFF)
 */

MEMORY {
    FLASH   : ORIGIN = 0x08000000, LENGTH = 1024K
    STAGING : ORIGIN = 0x08100000, LENGTH = 1024K
    RAM     : ORIGIN = 0x20000000, LENGTH = 768K  /* SRAM1 + SRAM2 + SRAM3 combined */
}

/* Stack and heap configuration */
_stack_start = ORIGIN(RAM) + LENGTH(RAM);
