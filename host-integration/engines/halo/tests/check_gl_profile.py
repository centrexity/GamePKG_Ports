"""Maintainer-only loader/query regression; no game media or GPU required."""
import argparse,hashlib,json,os,resource,subprocess
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--source',type=Path,required=True);p.add_argument('--sdl-include',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--cc',default='clang-18');p.add_argument('--cflag',action='append',default=[]);p.add_argument('--sanitize',action='store_true');a=p.parse_args();resource.setrlimit(resource.RLIMIT_CORE,(0,0));root=Path(__file__).resolve().parent;original=(a.source/'port/linux/src/gl_functions.c').read_text();begin=original.index('void gamepkg_gl_query_results(');end=original.index('static SDL_FunctionPointer gamepkg_gl_proc(',begin);body=original[begin:end]
cases={'positive':None,'offset':('offset + i * sizeof(GLuint)','offset + (i + 1) * sizeof(GLuint)','output[i]==expected'),'binding':('halo_glBindBuffer(GL_QUERY_BUFFER, (GLuint)previous);','(void)previous; /* lost state */','bound==17'),'driver-read':('previous = (GLint)gamepkg_query_binding;','halo_glGetIntegerv(GL_QUERY_BUFFER_BINDING, &previous);','driver_reads==0')};rows=[]
for name,fault in cases.items():
 work=a.output/name;work.mkdir(parents=True,exist_ok=True);text=original
 if fault:
  old,new,expected=fault;assert old in body;text=original[:begin]+body.replace(old,new,1)+original[end:]
 (work/'gl_functions.c').write_text(text);(work/'platform.h').write_text('#include <stdlib.h>\n#define TRUE 1\n#define FALSE 0\nvoid platform_log(const char *, ...);\n')
 cmd=[a.cc,'-std=gnu11','-O2','-g','-Wall','-Wextra','-Werror','-Wno-sign-compare',*a.cflag]
 if a.sanitize:cmd+=['-fsanitize=address,undefined','-fno-omit-frame-pointer']
 cmd+=['-I'+str(a.source/'port/linux/src'),'-I'+str(a.sdl_include),str(work/'gl_functions.c'),str(root/'gl_profile.c'),'-o',str(work/'check')]
 b=subprocess.run(cmd,capture_output=True,text=True,timeout=60);(work/'build.log').write_text(b.stdout+b.stderr);assert b.returncode==0,b.stderr
 r=subprocess.run([str(work/'check')],capture_output=True,text=True,timeout=30,env=dict(os.environ,ASAN_OPTIONS='detect_leaks=1:abort_on_error=1'));(work/'run.log').write_text(r.stdout+r.stderr)
 if fault:assert r.returncode!=0 and 'Assertion' in r.stderr and expected in r.stderr,r.stderr
 else:assert r.returncode==0 and 'PASS actual GL loader:' in r.stdout,r.stdout+r.stderr
 rows.append({'case':name,'exit':r.returncode,'passed':True})
(a.output/'result.json').write_text(json.dumps({'cases':rows,'sanitize':a.sanitize,'source_sha256':hashlib.sha256(original.encode()).hexdigest(),'scope':'Actual GL loader/adapters with fake driver; state, byte offsets, batching, capabilities and lack of pre-present driver queries. GPU pixels and shader compilation require separate hardware tests.'},indent=2)+'\n');print('PASS actual GL loader and3 exact-assertion negative controls')
