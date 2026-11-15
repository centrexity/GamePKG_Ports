// Candidate signed NVI1 template. Not selected by a recipe or admitted profile.
// Compile a fixed dimensionality wrapper; no runtime source generation.
#ifndef NVI1_CUBE_MASK
#define NVI1_CUBE_MASK 0
#endif
struct Nvi1Uniform { uint4 words[817]; };
[[vk::binding(0, 0)]] ConstantBuffer<Nvi1Uniform> nvi;
float4 fv(uint index) { return asfloat(nvi.words[min(index, 816u)]); }

struct VertexInput {
    [[vk::location(0)]] float4 v0:TEXCOORD0; [[vk::location(1)]] float4 v1:TEXCOORD1;
    [[vk::location(2)]] float4 v2:TEXCOORD2; [[vk::location(3)]] float4 v3:TEXCOORD3;
    [[vk::location(4)]] float4 v4:TEXCOORD4; [[vk::location(5)]] float4 v5:TEXCOORD5;
    [[vk::location(6)]] float4 v6:TEXCOORD6; [[vk::location(7)]] float4 v7:TEXCOORD7;
    [[vk::location(8)]] float4 v8:TEXCOORD8; [[vk::location(9)]] float4 v9:TEXCOORD9;
    [[vk::location(10)]] float4 v10:TEXCOORD10; [[vk::location(11)]] float4 v11:TEXCOORD11;
    [[vk::location(12)]] float4 v12:TEXCOORD12; [[vk::location(13)]] float4 v13:TEXCOORD13;
    [[vk::location(14)]] float4 v14:TEXCOORD14; [[vk::location(15)]] float4 v15:TEXCOORD15;
};
struct RasterInput {
    float4 position:SV_Position;
    [[vk::location(0)]] float4 diffuse:TEXCOORD0; [[vk::location(1)]] float4 specular:TEXCOORD1;
    [[vk::location(2)]] float4 t0:TEXCOORD2; [[vk::location(3)]] float4 t1:TEXCOORD3;
    [[vk::location(4)]] float4 t2:TEXCOORD4; [[vk::location(5)]] float4 t3:TEXCOORD5;
    [[vk::location(6)]] float fog:TEXCOORD6;
};

float4 operand(uint4 s, inout float4 r[13], inout float4 v[16], float4 position, int address) {
    float4 value=0;
    if(s.x==1u) value=s.y==12u?position:r[min(s.y,12u)];
    else if(s.x==2u) value=v[min(s.y,15u)];
    else if(s.x==3u) {
        int index=int(min(s.y,191u));
        if((s.w&2u)!=0u) index=clamp(address+index,0,191);
        value=fv(4u+uint(index));
    }
    float4 shuffled=float4(value[(s.z>>6)&3u],value[(s.z>>4)&3u],value[(s.z>>2)&3u],value[s.z&3u]);
    return (s.w&1u)!=0u?-shuffled:shuffled;
}
float4 masked(float4 old, float4 value, uint mask) {
    return float4((mask&8u)!=0u?value.x:old.x,(mask&4u)!=0u?value.y:old.y,
                  (mask&2u)!=0u?value.z:old.z,(mask&1u)!=0u?value.w:old.w);
}
void commit(uint destination,float4 value,inout float4 r[13],inout float4 o[13]) {
    uint temp=destination&15u, mask=(destination>>4)&15u;
    if(mask!=0u&&temp<=12u) {
        if(temp==12u) o[0]=masked(o[0],value,mask);
        else r[temp]=masked(r[temp],value,mask);
    }
    uint output=(destination>>8)&255u;
    mask=(destination>>16)&15u;
    if(mask!=0u&&output<=12u) o[output]=masked(o[output],value,mask);
}
float4 macValue(uint op,float4 a,float4 b,float4 c) {
    switch(op) {
        case 1:return a;
        case 2:return a*b;
        case 3:return a+c;
        case 4:return a*b+c;
        case 5:return dot(a.xyz,b.xyz).xxxx;
        case 6:return (dot(a.xyz,b.xyz)+b.w).xxxx;
        case 7:return dot(a,b).xxxx;
        case 8:return float4(1,a.y*b.y,a.z,b.w);
        case 9:return min(a,b);
        case 10:return max(a,b);
        case 11:return float4(a.x<b.x?1.0:0.0,a.y<b.y?1.0:0.0,a.z<b.z?1.0:0.0,a.w<b.w?1.0:0.0);
        case 12:return float4(a.x>=b.x?1.0:0.0,a.y>=b.y?1.0:0.0,a.z>=b.z?1.0:0.0,a.w>=b.w?1.0:0.0);
    }
    return 0;
}
float4 iluValue(uint op,float4 c) {
    float x=c.x;
    switch(op) {
        case 1:return c;
        case 2:return (1.0/x).xxxx;
        case 3:{
            float reciprocal=abs(1.0/x);
            reciprocal=reciprocal<=1.84467e19?max(reciprocal,5.42101e-20):1.84467e19;
            return ((asuint(x)>>31)!=0u?-reciprocal:reciprocal).xxxx;
        }
        case 4:return rsqrt(abs(x)).xxxx;
        case 5:{float integral=floor(x);return float4(exp2(integral),x-integral,exp2(x),1);}
        case 6:{
            float magnitude=abs(x);
            if(magnitude==0.0){
                // Produce the reference -infinity at runtime; Naga rejects
                // nonfinite SPIR-V constant literals in a prepared module.
                float negativeInfinity=-1.0/magnitude;
                return float4(negativeInfinity,1,negativeInfinity,1);
            }
            float exponent=floor(log2(magnitude));
            return float4(exponent,magnitude/exp2(exponent),log2(magnitude),1);
        }
        case 7:return float4(1,max(c.x,0.0),c.x>0.0?pow(max(c.y,0.0),clamp(c.w,-127.9961,127.9961)):0.0,1);
    }
    return 0;
}

[shader("vertex")]
RasterInput vertexMain(VertexInput input) {
    RasterInput result;
    result.position=float4(2,2,2,1); result.diffuse=0; result.specular=0;
    result.t0=0;result.t1=0;result.t2=0;result.t3=0;result.fog=1;
    if(any(nvi.words[0].xyz!=uint3(0x3149564e,1,13072)))return result;
    float4 v[16]={input.v0,input.v1,input.v2,input.v3,input.v4,input.v5,input.v6,input.v7,
                  input.v8,input.v9,input.v10,input.v11,input.v12,input.v13,input.v14,input.v15};
    uint mode=nvi.words[1].w&3u;
    float4 scale=fv(196),offset=fv(197);
    if(mode==2u) {
        if(nvi.words[1].x!=0u||nvi.words[1].z!=0x1e19u)return result;
        result.position=float4(v[0].xy*scale.xy+offset.xy,clamp(v[0].z*scale.z+scale.w,0.0,1.0),1);
        result.diffuse=v[3];result.specular=v[4];
        result.t0=v[9];result.t1=v[10];result.t2=v[11];result.t3=v[12];
        return result;
    }
    uint count=nvi.words[1].x;
    if(mode>1u||count==0u||count>136u)return result;
    float4 r[13],o[13];
    for(uint k=0;k<13;k++){r[k]=0;o[k]=float4(0,0,0,1);}
    int address=0;
    for(uint pc=0;pc<136;pc++) {
        if(pc>=count)break;
        uint base=201+pc*4;
        uint4 control=nvi.words[base];
        if(control.x>13u||control.y>7u)return result;
        float4 a=operand(nvi.words[base+1],r,v,o[0],address);
        float4 b=operand(nvi.words[base+2],r,v,o[0],address);
        float4 c=operand(nvi.words[base+3],r,v,o[0],address);
        // Both results precede every register/address write in this slot.
        float4 mac=macValue(control.x,a,b,c),ilu=iluValue(control.y,c);
        int nextAddress=address;
        if(control.x==13u) {
            if(isnan(a.x))return result; // unsupported dynamic ARL, never cast
            if(a.x>=192.0)nextAddress=192;
            else if(a.x<=-192.0)nextAddress=-192;
            else nextAddress=int(clamp(floor(a.x+0.001),-192.0,192.0));
        }
        if(control.x!=0u&&control.x!=13u)commit(control.z,mac,r,o);
        if(control.y!=0u)commit(control.w,ilu,r,o);
        address=nextAddress;
    }
    if(mode==1u) {
        float pw=abs(o[0].w)<(1.0/1.84467e19)
            ?asfloat((asuint(o[0].w)&0x80000000u)|asuint(1.0/1.84467e19)):o[0].w;
        float pz=(o[0].z*scale.z+scale.w)*pw;
        if(pz>pw&&pz<=pw*1.000001)pz=pw;
        result.position=float4((o[0].xy*scale.xy+offset.xy)*pw,pz,pw);
        uint4 fog=nvi.words[199],spec=nvi.words[200];
        float4 param=fv(198);
        float distance=o[5].x,factor=1;
        if(fog.x!=0u) {
            if(fog.y==0u)factor=param.x+distance*param.y-1.0;
            else if(fog.y==1u)factor=param.x+exp2(distance*param.y*16.0)-1.5;
            else factor=param.x+exp2(-distance*distance*param.y*param.y*32.0)-1.5;
            if(fog.z!=0u)factor=abs(factor);
            if((asuint(distance)&0x7fffffffu)==0x7f800000u||isnan(factor))factor=fog.w!=0u?1.0:0.0;
        }
        o[5]=factor.xxxx;
        if(spec.x==0u)o[4]=float4(0,0,0,1);
        else if(spec.y==0u)o[4].w=1;
    } else result.position=o[0];
    result.diffuse=saturate(o[3]);result.specular=saturate(o[4]);
    result.t0=o[9];result.t1=o[10];result.t2=o[11];result.t3=o[12];result.fog=o[5].x;
    return result;
}

#include "nvi1_fragment.slang"
