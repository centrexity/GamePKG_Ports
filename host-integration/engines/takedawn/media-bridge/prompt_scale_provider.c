/* SPDX-License-Identifier: Apache-2.0 */
#include "prompt_scale_provider.h"
#include <stdatomic.h>
static _Atomic(gamepkg_prompt_scale_fn) current;
void gamepkg_prompt_register_scale(gamepkg_prompt_scale_fn provider) {
    atomic_store_explicit(&current,provider,memory_order_release);
}
int gamepkg_prompt_scale(const uint32_t *source,uint32_t width,uint32_t height,unsigned factor,uint32_t *destination) {
    gamepkg_prompt_scale_fn provider=atomic_load_explicit(&current,memory_order_acquire);
    return provider?provider(source,width,height,factor,destination):0;
}
