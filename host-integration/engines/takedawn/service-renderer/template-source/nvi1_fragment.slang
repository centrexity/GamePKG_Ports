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
