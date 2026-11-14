#define main gamepkg_sdk_smoke_main
#include "native_smoke_initialized.c"
#undef main
extern int gamepkg_ui_unit_test(void),gamepkg_lod_unit_test(void),gamepkg_post_unit_test(void),gamepkg_fused_unit_test(void);
#ifdef GAMEPKG_POST_EDGE_MASK
extern int gamepkg_edge_mask_unit_test(void);
#endif
extern int gamepkg_cube_unit_test(void);
int main(int argc,char **argv){int rc=gamepkg_sdk_smoke_main(argc,argv);if(rc)return rc;
#ifdef GAMEPKG_POST_EDGE_MASK
rc=gamepkg_edge_mask_unit_test();if(rc)return rc;
#endif
rc=gamepkg_ui_unit_test();if(rc)return rc;rc=gamepkg_lod_unit_test();if(rc)return rc;rc=gamepkg_post_unit_test();if(rc)return rc;rc=gamepkg_fused_unit_test();return rc?rc:gamepkg_cube_unit_test();}
