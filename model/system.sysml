// System composition - connects actors via channels
package System {
    private import ScalarValues::*;

    part def PicoSystem {
        part button : ButtonActor;
        part control : RealtimeControlActor;
        part maintenance : MaintenanceActor;

        connection button.button_out to control.button_in, 4;
        connection maintenance.status_out to control.maintenance_in, 2;
    }
}
