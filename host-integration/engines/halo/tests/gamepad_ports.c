#include "../input/halo_gamepad_ports.h"
#include <assert.h>
#include <stdio.h>

int main(void)
{
    struct gamepkg_halo_ports state = {{0}, 0};
    uint64_t pads[4] = {UINT64_C(0x100000001), 0, 0, 0};
    int ports[4], i, j, step;
    gamepkg_halo_map_ports(&state, pads, 0, 0, ports);
    assert(ports[0] == 0 && ports[1] == -1);
    gamepkg_halo_map_ports(&state, pads, 1, 0, ports);
    assert(ports[0] == 0 && ports[1] == -1); /* held: defer */
    gamepkg_halo_map_ports(&state, pads, 1, 1, ports);
    assert(ports[0] == -1 && ports[1] == 0);
    pads[1] = UINT64_C(0x200000001); /* same low bits, another generation */
    gamepkg_halo_map_ports(&state, pads, 1, 0, ports);
    assert(ports[1] == 0 && ports[0] == 1); /* existing player stays put */
    pads[1] = 0;
    gamepkg_halo_map_ports(&state, pads, 1, 1, ports);
    assert(ports[1] == 0 && ports[0] == -1);
    pads[1] = UINT64_C(0x300000001);
    gamepkg_halo_map_ports(&state, pads, 1, 1, ports);
    assert(ports[1] == 0 && ports[0] == 1);
    pads[0] = 0;
    gamepkg_halo_map_ports(&state, pads, 1, 1, ports);
    assert(ports[0] == 1 && ports[1] == -1); /* surviving pad never compacts */
    gamepkg_halo_map_ports(&state, pads, 0, 0, ports);
    assert(ports[0] == 1);
    gamepkg_halo_map_ports(&state, pads, 0, 1, ports);
    gamepkg_halo_map_ports(&state, pads, 1, 0, ports);
    assert(ports[0] == 1);
    gamepkg_halo_map_ports(&state, pads, 1, 1, ports);
    assert(ports[1] == 1 && ports[0] == -1);
    /* Repeated topology changes, including sparse canonical slots. Every
     * surviving generation keeps its game port; no duplication or ghost pad.
     * No deliberate context change occurs during this phase. */
    for (step = 0; step < 4096; step++)
    {
        uint64_t before[4];
        for (i = 0; i < 4; i++) before[i] = state.devices[i];
        i = step % 4;
        pads[i] = pads[i] ? 0 : UINT64_C(0x100000000) + (uint64_t)step + 3;
        gamepkg_halo_map_ports(&state, pads, 1, 1, ports);
        for (i = 0; i < 4; i++)
        {
            int survives = 0;
            for (j = 0; j < 4; j++) if (pads[j] && pads[j] == before[i]) survives = 1;
            assert(!survives || state.devices[i] == before[i]);
            if (ports[i] < 0) assert(!state.devices[i]);
            else assert(ports[i] < 4 && state.devices[i] == pads[ports[i]]);
            for (j = 0; j < i; j++) assert(!state.devices[i] || state.devices[i] != state.devices[j]);
        }
        for (i = 0; i < 4; i++) if (pads[i])
        {
            int found = 0;
            for (j = 0; j < 4; j++) if (ports[j] == i) found++;
            assert(found == 1);
        }
    }
    puts("PASS Halo game ports: deferred context transitions and 4096 stable hotplug topologies");
    return 0;
}
