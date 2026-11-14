#!/usr/bin/env python3
"""Exercise actual engine path functions across two private RAM-cache launches."""
import argparse
import os
from pathlib import Path
import subprocess
import tempfile
import tomllib

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--source', required=True, type=Path)
parser.add_argument('--i686-sysroot', type=Path)
args = parser.parse_args()
root = Path(__file__).resolve().parents[1]
record = tomllib.loads((root / 'engine.toml').read_text())


def function(text, signature):
    start = text.index(signature)
    begin = text.index('{', start)
    depth = 1
    end = begin + 1
    while depth:
        depth += (text[end] == '{') - (text[end] == '}')
        end += 1
    return text[start:end]


translate = function((args.source / 'port/linux/src/xbox_files.c').read_text(),
                     'void platform_translate_path(')
cache_source = (args.source / 'source/cache/cache_files_windows.c').read_text()
# The prototype precedes the definition; select the definition explicitly.
cache = function(cache_source, 'static void cached_map_file_get_path(\n\tshort map_file_index,\n\tchar *path)\n{')
prefix = r'''
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>
typedef int BOOL;
struct posix_file_information { int unused; };
static const char *platform_data_root(void) { return "data"; }
static const char *platform_save_root(void) { return "saves"; }
static const char *platform_custom_edition_root(void) { return ""; }
static int posix_stat(const char *path, struct posix_file_information *unused)
{ struct stat st; (void)unused; return stat(path, &st); }
static int posix_make_directory(const char *path) { return mkdir(path, 0700); }
/* No pre-existing mixed-case names are present in this controlled fixture. */
static int posix_find_entry_case_insensitive(const char *a, const char *b, char *c, unsigned long d)
{ (void)a; (void)b; (void)c; (void)d; return 0; }
'''
suffix = r'''
static void write_file(const char *path, const char *data)
{ FILE *f = fopen(path, "wb"); assert(f); assert(fputs(data, f) >= 0); assert(!fclose(f)); }
static void read_file(const char *path, const char *expected)
{ char b[128] = {0}; FILE *f = fopen(path, "rb"); assert(f); assert(fread(b, 1, sizeof(b)-1, f) == strlen(expected)); assert(!strcmp(b, expected)); assert(!fclose(f)); }
int main(int argc, char **argv)
{
    char xbox[256], cache_path[1024], profile[1024], save[1024], last_solo[1024];
    assert(argc == 2);
    cached_map_file_get_path(0, xbox);
    platform_translate_path(xbox, cache_path, sizeof(cache_path));
    platform_translate_path("z:\\saved\\player_profiles\\default_profile\\00.sav", profile, sizeof(profile));
    platform_translate_path("z:\\savegame.bin", save, sizeof(save));
    platform_translate_path("z:\\last_solo.txt", last_solo, sizeof(last_solo));
    assert(!mkdir("saves/z/saved", 0700) || access("saves/z/saved", F_OK) == 0);
    assert(!mkdir("saves/z/saved/player_profiles", 0700) || access("saves/z/saved/player_profiles", F_OK) == 0);
    assert(!mkdir("saves/z/saved/player_profiles/default_profile", 0700) || access("saves/z/saved/player_profiles/default_profile", F_OK) == 0);
    assert(access(cache_path, F_OK) != 0); /* every launch starts with an empty cache */
    if (!strcmp(argv[1], "write"))
    {
        write_file(profile, "profile settings");
        write_file(save, "campaign checkpoint");
        write_file(last_solo, "last mission");
    }
    else
    {
        read_file(profile, "profile settings");
        read_file(save, "campaign checkpoint");
        read_file(last_solo, "last mission");
    }
    write_file(cache_path, "synthetic decompressed map");
    puts("PASS persistent profile/checkpoint/mission and isolated transient cache");
    return 0;
}
'''

with tempfile.TemporaryDirectory(prefix='gamepkg-save-partition-') as directory:
    work = Path(directory)
    (work / 'saves').mkdir()
    # Reuse the exact catalog partition, including its actual size.
    scratch, = record['launch']['scratch']
    mount = work / scratch['path']
    mount.mkdir(parents=True, exist_ok=True)
    flags = ['-g', '-O1', '-ffp-contract=off']
    if args.i686_sysroot:
        flags += ['-m32', '--sysroot=' + str(args.i686_sysroot.resolve())]
    else:
        flags += ['-fsanitize=address,undefined']
    env = dict(os.environ, ASAN_OPTIONS='detect_leaks=1:abort_on_error=1',
               UBSAN_OPTIONS='halt_on_error=1')
    for name, adapted, expected in [('actual', cache, True),
                                    ('wrong-cache-drive', cache.replace('q:', 'z:'), False)]:
        assert name == 'actual' or adapted != cache
        source = work / (name + '.c')
        binary = work / name
        source.write_text(prefix + translate + '\n' + adapted + suffix)
        subprocess.run(['clang', *flags, str(source), '-o', str(binary)], check=True)
        if name != 'actual':
            # Only our own synthetic persistent files are reset between controls.
            for file in (work / 'saves/z').rglob('*'):
                if file.is_file():
                    file.unlink()
        passed = True
        for phase in ['write', 'read']:
            command = ['bwrap', '--die-with-parent', '--ro-bind', '/', '/',
                       '--dev', '/dev', '--proc', '/proc', '--bind', str(work), str(work),
                       '--size', str(scratch['bytes']), '--tmpfs', str(mount),
                       '--chdir', str(work), str(binary), phase]
            result = subprocess.run(command, env=env, stdout=subprocess.PIPE,
                                    stderr=subprocess.PIPE, text=True)
            if result.returncode:
                passed = False
                # A broken cache route must fail the exact empty-cache check,
                # not merely fail compilation or a runtime setup prerequisite.
                if not expected:
                    assert 'access(cache_path, F_OK) != 0' in result.stderr, result.stderr
                else:
                    raise RuntimeError(result.stdout + result.stderr)
                break
        assert passed == expected, (name, passed)
        assert not any(mount.iterdir()), 'Cache data escaped the private mount'
        print('PASS', name if expected else name + ' rejected', flush=True)
