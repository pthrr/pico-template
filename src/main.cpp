#include <cstdio>

#include "pico/multicore.h"
#include "pico/stdlib.h"

void core1_main()
{
    stdio_usb_init();

    while( true ) {
        printf( "Core 1: Hello from Core 1!\n" );
        sleep_ms( 1000 );
    }
}

auto main() -> int
{
    stdio_init_all();
    multicore_launch_core1( core1_main );

    while( true ) {
        volatile int aaa = 1;
        printf( "Core 0: Hello from Core 0!\n" );
        sleep_ms( 1000 );
    }

    return 0;
}
