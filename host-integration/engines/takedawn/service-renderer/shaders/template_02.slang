// Prepared by maintainer before signing; never expanded by the game.
#define NVI1_CUBE_MASK 2
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

// Included by nvi1.slang; fixed resource dimensions are signed wrapper inputs.
#if (NVI1_CUBE_MASK & 1)
[[vk::binding(1,0)]] TextureCube<float4> texture0;
#else
[[vk::binding(1,0)]] Texture2D<float4> texture0;
#endif
[[vk::binding(2,0)]] SamplerState sampler0;
#if (NVI1_CUBE_MASK & 2)
[[vk::binding(3,0)]] TextureCube<float4> texture1;
#else
[[vk::binding(3,0)]] Texture2D<float4> texture1;
#endif
[[vk::binding(4,0)]] SamplerState sampler1;
#if (NVI1_CUBE_MASK & 4)
[[vk::binding(5,0)]] TextureCube<float4> texture2;
#else
[[vk::binding(5,0)]] Texture2D<float4> texture2;
#endif
[[vk::binding(6,0)]] SamplerState sampler2;
#if (NVI1_CUBE_MASK & 8)
[[vk::binding(7,0)]] TextureCube<float4> texture3;
#else
[[vk::binding(7,0)]] Texture2D<float4> texture3;
#endif
[[vk::binding(8,0)]] SamplerState sampler3;

float4 sampleStage(uint stage,float4 tc,float2 uv,float bias) {
    switch(stage) {
#if (NVI1_CUBE_MASK & 1)
        case 0:return texture0.SampleBias(sampler0,tc.xyz,bias);
#else
        case 0:return texture0.SampleBias(sampler0,uv,bias);
#endif
#if (NVI1_CUBE_MASK & 2)
        case 1:return texture1.SampleBias(sampler1,tc.xyz,bias);
#else
        case 1:return texture1.SampleBias(sampler1,uv,bias);
#endif
#if (NVI1_CUBE_MASK & 4)
        case 2:return texture2.SampleBias(sampler2,tc.xyz,bias);
#else
        case 2:return texture2.SampleBias(sampler2,uv,bias);
#endif
#if (NVI1_CUBE_MASK & 8)
        case 3:return texture3.SampleBias(sampler3,tc.xyz,bias);
#else
        case 3:return texture3.SampleBias(sampler3,uv,bias);
#endif
    }
    return 0;
}
float mapInput(float x,uint mapping) {
    switch(mapping) {
        case 0:return max(x,0.0);
        case 1:return 1.0-saturate(x);
        case 2:return 2.0*max(x,0.0)-1.0;
        case 3:return 1.0-2.0*max(x,0.0);
        case 4:return max(x,0.0)-0.5;
        case 5:return 0.5-max(x,0.0);
        case 6:return x;
        case 7:return -x;
    }
    return 0;
}
float3 rgbInput(uint descriptor,inout float4 r[16],bool finalInput) {
    float4 source=r[descriptor&15u];
    float3 value=(descriptor&16u)!=0u?source.aaa:source.rgb;
    uint mapping=(descriptor>>5)&7u;
    if(finalInput)return mapping==1u?1.0-saturate(value):saturate(value);
    return float3(mapInput(value.x,mapping),mapInput(value.y,mapping),mapInput(value.z,mapping));
}
float alphaInput(uint descriptor,inout float4 r[16],bool finalInput) {
    float4 source=r[descriptor&15u];
    float value=(descriptor&16u)!=0u?source.a:source.b;
    uint mapping=(descriptor>>5)&7u;
    if(finalInput)return mapping==1u?1.0-saturate(value):saturate(value);
    return mapInput(value,mapping);
}
float3 outputMap(float3 value,uint mapping) {
    if(mapping==1u)value-=0.5;
    else if(mapping==2u)value*=2;
    else if(mapping==3u)value=(value-0.5)*2;
    else if(mapping==4u)value*=4;
    else if(mapping==6u)value*=0.5;
    return clamp(value,-1.0,1.0);
}
float signedBump(float value) {
    value*=255.0;
    return value>=128.0?(value-256.0)/127.0:value/127.0;
}
bool passesAlpha(float alpha) {
    uint4 state=nvi.words[3];
    if(state.z==0u)return true;
    if(isnan(alpha))return false; // never convert NaN to an integer
    uint a=uint(saturate(alpha)*255.0+0.5),reference=uint(asfloat(state.x)*255.0+0.5);
    switch(state.y) {
        case 1:return false;
        case 2:return a<reference;
        case 3:return a==reference;
        case 4:return a<=reference;
        case 5:return a>reference;
        case 6:return a!=reference;
        case 7:return a>=reference;
        case 8:return true;
    }
    return false;
}

[shader("fragment")]
float4 fragmentMain(RasterInput input):SV_Target {
    if(any(nvi.words[0].xyz!=uint3(0x3149564e,1,13072)))discard;
    float4 tc[4]={input.t0,input.t1,input.t2,input.t3};
    uint4 modes=nvi.words[798];
    float4 biases=fv(807);
    if((nvi.words[1].w&4u)!=0u) {
        // Preserve the legacy basic path's supported NONE/2D/PASSTHRU cases.
        float4 color=input.diffuse;
        if(modes.x==4u)color*=saturate(tc[0]);
        else if(modes.x==1u) {
            float2 uv=tc[0].xy;
            if(tc[0].w!=0.0&&tc[0].w!=1.0)uv/=tc[0].w;
            color*=sampleStage(0,tc[0],uv*fv(803).xy,biases.x);
        } else if(modes.x!=0u)discard;
        if(!passesAlpha(color.a))discard;
        return color;
    }
    uint count=nvi.words[1].y;
    if(count>8u)discard;
    float4 r[16];for(uint k=0;k<16;k++)r[k]=0;
    float4 fog=fv(797);
    r[3]=nvi.words[2].w!=0u?float4(fog.rgb,saturate(input.fog)):fog;
    r[4]=input.diffuse;r[5]=input.specular;
    bool clip=false;
    for(uint k=0;k<4;k++) {
        uint mode=modes[k];
        if(mode>7u||mode==2u)discard;
        if(mode==0u){r[8+k]=float4(0,0,0,1);continue;}
        if(mode==4u){r[8+k]=saturate(tc[k]);continue;}
        if(mode==5u) {
            uint mask=nvi.words[801][k];
            if(mask>15u)discard;
            for(uint component=0;component<4;component++) {
                bool positive=tc[k][component]>=0.0;
                clip=clip||((mask&(1u<<component))!=0u?positive:tc[k][component]<0.0);
            }
            continue;
        }
        float2 uv=tc[k].xy;
        uint previous=nvi.words[800][k];
        if(mode==6u||mode==7u) {
            if(k==0u||previous>=k)discard;
            uint sign=nvi.words[799][previous];
            float4 prior=r[8+previous];
            float2 dsdt=(sign&3u)==3u?prior.bg:float2(signedBump(prior.b),signedBump(prior.g));
            float4 matrix=fv(808+k);
            uv+=float2(matrix.x*dsdt.x+matrix.z*dsdt.y,matrix.y*dsdt.x+matrix.w*dsdt.y);
        } else if(mode==1u&&tc[k].w!=0.0&&tc[k].w!=1.0)uv/=tc[k].w;
        float4 sampled=sampleStage(k,tc[k],uv*fv(803+k).xy,biases[k]);
        if(mode==7u) {
            float luminance=r[8+previous].r;
            if((nvi.words[799][previous]&4u)!=0u)luminance=luminance>=0.0?luminance*0.5:1.0+luminance*0.5;
            float4 parameters=fv(812+k);
            sampled*=parameters.x*luminance+parameters.y;
        }
        uint sign=nvi.words[799][k];
        if((sign&1u)!=0u)sampled.b=max((sampled.b*255.0-128.0)/127.0,-1.0);
        if((sign&2u)!=0u)sampled.g=max((sampled.g*255.0-128.0)/127.0,-1.0);
        if((sign&4u)!=0u)sampled.r=max((sampled.r*255.0-128.0)/127.0,-1.0);
        if((sign&8u)!=0u)sampled.a=max((sampled.a*255.0-128.0)/127.0,-1.0);
        if(nvi.words[802][k]!=0u)sampled.rgb=1;
        r[8+k]=sampled;
    }
    // Delay clip discard until sampling has completed, preserving derivative
    // participation for neighboring lanes that survive the clip plane.
    if(clip)discard;
    r[12].a=r[8].a;
    for(uint stage=0;stage<8;stage++) {
        if(stage>=count)break;
        r[1]=fv(779+(nvi.words[2].x!=0u?stage:0));
        r[2]=fv(787+(nvi.words[2].y!=0u?stage:0));
        uint base=745+stage*4;
        uint4 ri=nvi.words[base],ai=nvi.words[base+1],ro=nvi.words[base+2],ao=nvi.words[base+3];
        if(any(ro.xyz>13u)||any(ao.xyz>13u))discard;
        float3 a=rgbInput(ri.x,r,false),b=rgbInput(ri.y,r,false);
        float3 c=rgbInput(ri.z,r,false),d=rgbInput(ri.w,r,false);
        float aa=alphaInput(ai.x,r,false),ab=alphaInput(ai.y,r,false);
        float ac=alphaInput(ai.z,r,false),ad=alphaInput(ai.w,r,false);
        bool select=false;
        if(((ro.w|ao.w)&4u)!=0u){
            if(isnan(r[12].a))discard;
            select=nvi.words[2].z!=0u?r[12].a>=0.5:((uint(saturate(r[12].a)*255.0+0.5)&1u)!=0u);
        }
        float3 rgbAB=(ro.w&1u)!=0u?dot(a,b).xxx:a*b;
        float3 rgbCD=(ro.w&2u)!=0u?dot(c,d).xxx:c*d;
        float3 rgbSum=(ro.w&4u)!=0u?(select?rgbCD:rgbAB):rgbAB+rgbCD;
        uint mapping=(ro.w>>8)&7u;
        rgbAB=outputMap(rgbAB,mapping);rgbCD=outputMap(rgbCD,mapping);rgbSum=outputMap(rgbSum,mapping);
        float alphaAB=aa*ab,alphaCD=ac*ad;
        float alphaSum=(ao.w&4u)!=0u?(select?alphaCD:alphaAB):alphaAB+alphaCD;
        uint alphaMapping=(ao.w>>8)&7u;
        // Every input/product above is evaluated before the first write.
        if(ro.x!=0u)r[ro.x].rgb=rgbAB;
        if(ro.y!=0u)r[ro.y].rgb=rgbCD;
        if(ro.z!=0u)r[ro.z].rgb=rgbSum;
        if(ao.x!=0u)r[ao.x].a=outputMap(alphaAB.xxx,alphaMapping).x;
        if(ao.y!=0u)r[ao.y].a=outputMap(alphaCD.xxx,alphaMapping).x;
        if(ao.z!=0u)r[ao.z].a=outputMap(alphaSum.xxx,alphaMapping).x;
        if((ro.w&8u)!=0u&&ro.x!=0u)r[ro.x].a=rgbAB.b;
        if((ro.w&16u)!=0u&&ro.y!=0u)r[ro.y].a=rgbCD.b;
    }
    r[1]=fv(795);r[2]=fv(796);
    uint4 abcd=nvi.words[777],efg=nvi.words[778];
    float3 e=rgbInput(efg.x,r,true),f=rgbInput(efg.y,r,true);
    float3 v1=saturate(r[5].rgb),r0=saturate(r[12].rgb);
    uint flags=nvi.words[0].w;
    if((flags&0x40u)!=0u)v1=1.0-v1;
    if((flags&0x20u)!=0u)r0=1.0-r0;
    float3 sum=v1+r0;
    if((flags&0x80u)!=0u)sum=saturate(sum);
    r[14]=float4(sum,0);r[15]=float4(e*f,0);
    float3 a=rgbInput(abcd.x,r,true),b=rgbInput(abcd.y,r,true);
    float3 c=rgbInput(abcd.z,r,true),d=rgbInput(abcd.w,r,true);
    float4 color=float4(saturate(a*b+(1.0-a)*c+d),alphaInput(efg.z,r,true));
    if(!passesAlpha(color.a))discard;
    return color;
}

