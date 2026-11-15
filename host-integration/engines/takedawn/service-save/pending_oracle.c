/* Maintainer-only synthetic oracle. pending.inc is extracted, unchanged, from
 * the pinned saveguard. This harness is never linked into a game object. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
typedef char xbox_host_char;
#define MAX_PATH 4096
#define ENTRIES 16
#define SG_TRIES 4
#define PEND_LEN 6
#define LOGP "save: "
#define LEAF_FMT "%s"
#define LOCK() ((void)0)
#define UNLOCK() ((void)0)
#define hlen strlen
#define hcmp strcmp
static const char k_pend[] = ".~pend";
typedef struct {
    int used, tries;
    unsigned code;
    char real[MAX_PATH], pend[MAX_PATH];
} pend_entry;
static pend_entry s_pend[ENTRIES];
static char s_problem[400];
static int s_problem_seq, s_problem_on, injected_failure;
static const char *leaf(const char *name) { return name; }
static void hcpy(char *out, size_t n, const char *in) {
    if (strlen(in) >= n) abort();
    memcpy(out, in, strlen(in) + 1);
}
static void reason_text(unsigned code, char *out, size_t n) {
    (void)code;
    if (n < 8) abort();
    memcpy(out, "fixture", 8);
}
static int h_exists(const char *name) { (void)name; return 1; }
static void h_unlink(const char *name) { (void)name; }
static int h_replace(const char *from, const char *to, unsigned *code) {
    (void)from; (void)to; (void)code;
    return 1;
}
static int put_in_place(const char *from, const char *to, unsigned *code) {
    (void)from; (void)to;
    *code = 3;
    return !injected_failure;
}
#include "pending.inc"
int main(void) {
    char operation;
    unsigned key, failure;
    while (scanf(" %c %u %u", &operation, &key, &failure) == 3) {
        if (key > 3 || failure > 1) return 1;
        if (operation == 'Z') {
            memset(s_pend, 0, sizeof s_pend);
            s_problem_seq = s_problem_on = 0;
        } else if (operation == 'X') {
            injected_failure = (int)failure;
            pend_retry(1);
        } else if (operation == 'A' || operation == 'S') {
            int due = 0;
            char real[32], shadow[40];
            for (int i = 0; i < ENTRIES; ++i)
                due |= s_pend[i].used && s_pend[i].tries < SG_TRIES;
            snprintf(real, sizeof real, "save%u", key);
            snprintf(shadow, sizeof shadow, "%s.~new", real);
            if (operation == 'A') pend_add(real, shadow, 3);
            else {
                pend_entry *p = pend_of(real);
                if (p) pend_drop(p, 1);
            }
            if (due) {
                injected_failure = (int)failure;
                pend_retry(0);
            }
        } else return 2;
        unsigned mask = 0, count = 0;
        for (unsigned i = 0; i < 4; ++i) {
            char real[32];
            snprintf(real, sizeof real, "save%u", i);
            if (pend_of(real)) { mask |= 1u << i; ++count; }
        }
        printf("%u %u %u %u\n", count, (unsigned)s_problem_on,
               s_problem_on ? (unsigned)s_problem_seq : 0u, mask);
    }
    return 0;
}
