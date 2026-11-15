/* Maintainer oracle only. Links the unchanged pinned portable C parsers. */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include "d3d8_vsh_parse.h"
#include "d3d8_combiners_parse.h"
#include "nv2a_combiner.h"
#include <string.h>

static uint32_t read32(void) {
    uint8_t b[4];
    if (fread(b, 1, 4, stdin) != 4) exit(2);
    return (uint32_t)b[0] | (uint32_t)b[1]<<8 | (uint32_t)b[2]<<16 | (uint32_t)b[3]<<24;
}
static void write32(uint32_t v) {
    uint8_t b[4] = {(uint8_t)v,(uint8_t)(v>>8),(uint8_t)(v>>16),(uint8_t)(v>>24)};
    if (fwrite(b, 1, 4, stdout) != 4) exit(3);
}
static uint32_t dest(const NV2AVshDstOperand *d) {
    return (d->write_mask ? (uint32_t)d->temp_reg : 15u)
        | (uint32_t)d->write_mask << 4
        | (d->out_mask ? (uint32_t)d->output_reg : 255u) << 8
        | (uint32_t)d->out_mask << 16;
}
static int used(int mac, int ilu, int operand) {
    if (operand == 0) return mac != 0;
    if (operand == 1) return mac == 2 || (mac >= 4 && mac <= 12);
    return mac == 3 || mac == 4 || ilu != 0;
}
static void vertex(void) {
    uint32_t raw[4]; NV2AVshProgram p;
    for (int i=0;i<4;i++) raw[i]=read32();
    d3d8_vsh_parse(raw,1,&p);
    const NV2AVshInstruction *i=&p.insns[0];
    write32((uint32_t)i->mac_op); write32((uint32_t)i->ilu_op);
    write32(dest(&i->mac_dst)); write32(dest(&i->ilu_dst));
    for (int k=0;k<3;k++) {
        const NV2AVshSrcOperand *s=&i->mac_src[k];
        int active=used(i->mac_op,i->ilu_op,k) && s->reg_type!=NV2A_VSH_REG_COUNT;
        write32(active?(uint32_t)s->reg_type+1u:0u);
        write32(active?(uint32_t)s->reg_index:0u);
        write32(active?(uint32_t)(s->swizzle.x<<6 | s->swizzle.y<<4 | s->swizzle.z<<2 | s->swizzle.w):0u);
        write32(active?(uint32_t)(s->negate | s->rel_addr<<1):0u);
    }
    write32(p.inputs_read);
}
static uint32_t input(const NV2ACombinerInput *i) {
    return (uint32_t)i->reg | (uint32_t)i->alpha_rep<<4 | (uint32_t)i->mapping<<5;
}
static void output(const NV2ACombinerOutput *o) {
    write32((uint32_t)o->ab_dst);write32((uint32_t)o->cd_dst);write32((uint32_t)o->sum_dst);
    write32((uint32_t)o->ab_dot | (uint32_t)o->cd_dot<<1 | (uint32_t)o->mux_sum<<2
        | (uint32_t)o->ab_blue_to_alpha<<3 | (uint32_t)o->cd_blue_to_alpha<<4
        | (uint32_t)o->output_map<<8);
}
static void combiner(void) {
    uint32_t ci[8],ai[8],co[8],ao[8];NV2ACombinerState s;
    uint32_t control=read32(),f0=read32(),f1=read32();
    for(int i=0;i<8;i++){ci[i]=read32();ai[i]=read32();co[i]=read32();ao[i]=read32();}
    d3d8_combiners_from_regs(ci,ai,co,ao,control,f0,f1,0,&s);
    write32((uint32_t)s.num_stages);write32((uint32_t)s.uniq_c0);
    write32((uint32_t)s.uniq_c1);write32((uint32_t)s.mux_msb);write32(s.final_flags);
    for(int i=0;i<8;i++) {
        for(int k=0;k<4;k++) write32(input(&s.stages[i].rgb_input[k]));
        for(int k=0;k<4;k++) write32(input(&s.stages[i].alpha_input[k]));
        output(&s.stages[i].rgb_output);output(&s.stages[i].alpha_output);
    }
    for(int k=0;k<7;k++) write32(input(&s.final_input[k]));
}
static void evaluate_combiner(void) {
    Nv2aCombiner rc={0};float registers[16][4],result[4];
    rc.control=read32();rc.final0=read32();rc.final1=read32();
    rc.final_c0=read32();rc.final_c1=read32();
    for(int i=0;i<8;i++) {
        rc.color_icw[i]=read32();rc.alpha_icw[i]=read32();
        rc.color_ocw[i]=read32();rc.alpha_ocw[i]=read32();
        rc.factor0[i]=read32();rc.factor1[i]=read32();
    }
    for(int i=0;i<16;i++)for(int k=0;k<4;k++){uint32_t bits=read32();memcpy(&registers[i][k],&bits,4);}
    nv2a_rc_eval_regs(&rc,registers,result,NULL);
    for(int k=0;k<4;k++){uint32_t bits;memcpy(&bits,&result[k],4);write32(bits);}
}
int main(void) {
    for (;;) {
        int kind=fgetc(stdin);
        if(kind==EOF) return ferror(stdin)?4:0;
        if(kind==0) vertex(); else if(kind==1) combiner(); else if(kind==2)evaluate_combiner(); else return 5;
    }
}
