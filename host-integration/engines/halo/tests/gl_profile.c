/* Asset-free test of the actual renderer's GL loader and query adapters. */
#define GL_FUNCTIONS_DEFINE
#include "gl.h"
#include <SDL3/SDL.h>
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdarg.h>
static int minor,missing,driver_reads,dsa_calls,begin_calls;static GLuint bound,next_query,active,query_target[16384],output[16384];
static const char *extensions[]={"GL_ARB_clip_control","GL_ARB_buffer_storage","GL_ARB_query_buffer_object","GL_ARB_multi_bind"};
void platform_log(const char *fmt, ...) {(void)fmt;}
bool SDL_GL_GetAttribute(SDL_GLAttr attr,int *out) {*out=attr==SDL_GL_CONTEXT_MAJOR_VERSION?4:minor;return true;}
bool SDL_GL_ExtensionSupported(const char *name) {for(int i=0;i<4;i++)if(!strcmp(name,extensions[i]))return missing!=i+1;return false;}
static void noop(void) {}
static void GLAPIENTRY bind_buffer(GLenum target,GLuint buffer) {assert(target==GL_QUERY_BUFFER);bound=buffer;}
static void GLAPIENTRY get_integer(GLenum name,GLint *out) {assert(name==GL_QUERY_BUFFER_BINDING);driver_reads++;*out=bound;}
static void GLAPIENTRY gen_queries(GLsizei count,GLuint *queries) {for(int i=0;i<count;i++){assert(next_query<16384);queries[i]=next_query++;}}
static void GLAPIENTRY begin_query(GLenum target,GLuint query) {assert(!active&&query&&query<16384);active=query;query_target[query]=target;begin_calls++;}
static void GLAPIENTRY end_query(GLenum target) {assert(active&&query_target[active]==target);active=0;}
static void GLAPIENTRY create_queries(GLenum target,GLsizei count,GLuint *queries) {dsa_calls++;gen_queries(count,queries);for(int i=0;i<count;i++)query_target[queries[i]]=target;}
static void write_result(GLuint query,GLenum pname,GLintptr offset) {assert(query&&query<next_query&&pname==GL_QUERY_RESULT);assert(offset>=0&&offset%sizeof(GLuint)==0&&(uintptr_t)offset/sizeof(GLuint)<16384);output[(uintptr_t)offset/sizeof(GLuint)]=query+101;}
static void GLAPIENTRY query_buffer(GLuint query,GLuint buffer,GLenum pname,GLintptr offset) {assert(buffer==33);write_result(query,pname,offset);}
static void GLAPIENTRY query_object(GLuint query,GLenum pname,GLuint *offset) {assert(bound==33);write_result(query,pname,(GLintptr)offset);}
SDL_FunctionPointer SDL_GL_GetProcAddress(const char *name) {
#define PROC(n,f) if(!strcmp(name,n))return (SDL_FunctionPointer)f
 PROC("glBindBuffer",bind_buffer);PROC("glGetIntegerv",get_integer);PROC("glGenQueries",gen_queries);PROC("glBeginQuery",begin_query);PROC("glEndQuery",end_query);PROC("glCreateQueries",create_queries);PROC("glGetQueryBufferObjectuiv",query_buffer);PROC("glGetQueryObjectuiv",query_object);
 return noop;
}
static void run(int version,int force) {
 minor=version;missing=0;bound=0;next_query=1;active=0;driver_reads=dsa_calls=begin_calls=0;memset(query_target,0,sizeof(query_target));setenv("GAMEPKG_GL43",force?"1":"",1);assert(gl_functions_load());assert(!strcmp(gamepkg_gl_shader_version(),version==3||force?"430 core":"450 core"));
 GLuint queries[12288];halo_glCreateQueries(GL_SAMPLES_PASSED,12288,queries);assert(next_query==12289&&!active);for(int i=0;i<12288;i++){assert(queries[i]==(GLuint)i+1);assert(query_target[queries[i]]==GL_SAMPLES_PASSED);}assert(version==3||force?begin_calls==12288&&dsa_calls==0:begin_calls==0&&dsa_calls==1);
 const int counts[]={0,1,2,4096,12288};for(unsigned c=0;c<sizeof(counts)/sizeof(counts[0]);c++)for(unsigned offset=0;offset<4;offset++) {
  for(int i=0;i<16384;i++)output[i]=0xaabbccdd;halo_glBindBuffer(GL_QUERY_BUFFER,17);driver_reads=0;gamepkg_gl_query_results(queries,33,GL_QUERY_RESULT,offset*sizeof(GLuint),counts[c]);assert(bound==17);assert(driver_reads==0);
  for(unsigned i=0;i<16384;i++){GLuint expected=i>=offset&&i<offset+counts[c]?queries[i-offset]+101:0xaabbccdd;assert(output[i]==expected);}
 }
 for(int i=1;i<=4;i++){minor=3;missing=i;assert(!gl_functions_load());}missing=0;
}
int main(void) {run(5,0);run(3,0);run(5,1);puts("PASS actual GL loader: default/automatic/forced profiles, 36864 query objects,60 batch shapes, exact results/bindings, zero driver reads,4 required-extension refusals per profile");return 0;}
