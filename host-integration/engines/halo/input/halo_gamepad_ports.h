/* GamePKG host glue: preserve Halo's keyboard/gamepad split-screen policy
 * without reassigning an existing pad when another one joins or leaves.
 * IDs are the shared service's generation handles, not SDL pointers/indices.
 * The caller owns this state on the existing SDL thread. */
#ifndef GAMEPKG_HALO_GAMEPAD_PORTS_H
#define GAMEPKG_HALO_GAMEPAD_PORTS_H
#include <stdint.h>

struct gamepkg_halo_ports {
    uint64_t devices[4];
    int lone_split;
};

/* The incoming array retains canonical player-slot holes. Output entries
 * select incoming indices, or -1 for an empty game port. A context change
 * can move the lone controller only at rest, as in Halo's original mapper.
 * Hotplug by itself never moves a surviving controller. */
static void gamepkg_halo_map_ports(struct gamepkg_halo_ports *state,
    const uint64_t incoming[4], int split, int lone_idle, int output[4])
{
    int i, j, count = 0, single = -1, present, assigned, target;
    for (i = 0; i < 4; i++)
    {
        if (incoming[i]) { count++; single = i; }
        output[i] = -1;
        present = 0;
        for (j = 0; j < 4; j++)
            if (incoming[j] && incoming[j] == state->devices[i]) present = 1;
        if (!present) state->devices[i] = 0;
    }
    if (count == 1 && (split != 0) != state->lone_split && lone_idle)
    {
        state->lone_split = split != 0;
        for (i = 0; i < 4; i++) state->devices[i] = 0;
        state->devices[state->lone_split ? 1 : 0] = incoming[single];
    }
    for (i = 0; i < 4; i++)
    {
        if (!incoming[i]) continue;
        assigned = 0;
        for (j = 0; j < 4; j++)
            if (state->devices[j] == incoming[i]) assigned = 1;
        if (assigned) continue;
        target = count == 1 ? (state->lone_split ? 1 : 0) : i;
        if (state->devices[target])
            for (target = 0; target < 4 && state->devices[target]; target++) {}
        if (target < 4) state->devices[target] = incoming[i];
    }
    for (i = 0; i < 4; i++)
        for (j = 0; j < 4; j++)
            if (state->devices[i] && state->devices[i] == incoming[j]) output[i] = j;
}
#endif
