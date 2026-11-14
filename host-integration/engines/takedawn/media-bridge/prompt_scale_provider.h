/* SPDX-License-Identifier: Apache-2.0 */
#ifndef GAMEPKG_PROMPT_SCALE_PROVIDER_H
#define GAMEPKG_PROMPT_SCALE_PROVIDER_H
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
typedef int (*gamepkg_prompt_scale_fn)(const uint32_t*,uint32_t,uint32_t,unsigned,uint32_t*);
void gamepkg_prompt_register_scale(gamepkg_prompt_scale_fn provider);
int gamepkg_prompt_scale(const uint32_t*,uint32_t,uint32_t,unsigned,uint32_t*);
#ifdef __cplusplus
}
#endif
#endif
