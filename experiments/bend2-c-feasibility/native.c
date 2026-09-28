// Same lexical rules and token-buffer/fold stages as frontend.bend, stored
// compactly in C arrays. pthread workers claim files dynamically like a build.
#include <pthread.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

typedef struct {
  unsigned char *data;
  uint32_t len;
} Source;
typedef struct {
  uint32_t kind, value;
} Token;
typedef struct {
  uint32_t value, op;
} Frame;
static Source *files;
static uint32_t count;
static atomic_uint next;
static int folding;
static uint64_t stamp(void) {
  struct timespec t;
  clock_gettime(CLOCK_MONOTONIC, &t);
  return (uint64_t)t.tv_sec * 1000000 + t.tv_nsec / 1000;
}
static uint32_t read32(FILE *f) {
  unsigned char b[4];
  if (fread(b, 1, 4, f) != 4)
    exit(2);
  return b[0] | (uint32_t)b[1] << 8 | (uint32_t)b[2] << 16 |
         (uint32_t)b[3] << 24;
}
static void load(const char *p) {
  FILE *f = fopen(p, "rb");
  if (!f) {
    perror(p);
    exit(2);
  }
  count = read32(f);
  files = calloc(count, sizeof(*files));
  if (!files)
    exit(2);
  for (uint32_t i = 0; i < count; i++) {
    files[i].len = read32(f);
    files[i].data = malloc(files[i].len);
    if (!files[i].data ||
        fread(files[i].data, 1, files[i].len, f) != files[i].len)
      exit(2);
  }
  fclose(f);
}
static uint32_t apply(uint32_t op, uint32_t a, uint32_t b) {
  return op == '+' ? a + b : op == '*' ? a * b : op == '^' ? a ^ b : b;
}
static uint32_t process(Source s) {
  Token *ts = malloc((s.len + 1) * sizeof(*ts));
  if (!ts)
    exit(2);
  uint32_t n = 0, mode = 0, v = 0;
  for (uint32_t i = 0; i < s.len; i++) {
    uint32_t c = s.data[i];
    if (c >= '0' && c <= '9') {
      v = mode == 1 ? v * 33 + c : mode == 2 ? v * 10 + c - '0' : c - '0';
      mode = mode == 1 ? 1 : 2;
    } else if ((c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || c == '_') {
      if (mode == 1)
        v = v * 33 + c;
      else if (mode == 2) {
        ts[n++] = (Token){2, v};
        mode = 0;
        v = 0;
      } else {
        mode = 1;
        v = c;
      }
    } else {
      if (mode)
        ts[n++] = (Token){mode, v};
      mode = 0;
      v = 0;
      if (c > 32)
        ts[n++] = (Token){3, c};
    }
  }
  if (mode)
    ts[n++] = (Token){mode, v};
  uint32_t result = 0;
  if (!folding) {
    for (uint32_t i = 0; i < n; i++)
      result = result * 16777619 + (ts[i].value ^ (ts[i].kind == 1   ? 268435456
                                                   : ts[i].kind == 2 ? 536870912
                                                                     : 0));
  } else {
    Frame *st = malloc((n + 1) * sizeof(*st));
    if (!st)
      exit(2);
    uint32_t active = 0, acc = 0, op = 0, sp = 0;
    for (uint32_t i = 0; i < n; i++) {
      uint32_t c = ts[i].value;
      if (ts[i].kind == 1) {
        uint32_t h = 0;
        const char *r = "return";
        while (*r)
          h = h * 33 + (unsigned char)*r++;
        active |= c == h;
      } else if (ts[i].kind == 2) {
        acc = apply(op, acc, c);
        op = 0;
      } else if (active) {
        if (c == '(') {
          st[sp++] = (Frame){acc, op};
          acc = op = 0;
        } else if (c == ')') {
          if (sp) {
            Frame f = st[--sp];
            acc = apply(f.op, f.value, acc);
          }
          op = 0;
        } else if (c == ';') {
          result += acc;
          active = acc = op = sp = 0;
        } else
          op = c;
      }
    }
    free(st);
  }
  free(ts);
  return result;
}
static void *worker(void *arg) {
  uint32_t sum = 0, i;
  while ((i = atomic_fetch_add_explicit(&next, 1, memory_order_relaxed)) <
         count)
    sum += process(files[i]);
  *(uint32_t *)arg = sum;
  return NULL;
}
static uint32_t run(unsigned threads) {
  pthread_t ids[128];
  uint32_t sums[128] = {0}, sum = 0;
  atomic_store(&next, 0);
  if (threads == 1)
    worker(&sums[0]);
  else {
    for (unsigned t = 0; t < threads; t++)
      pthread_create(&ids[t], NULL, worker, &sums[t]);
    for (unsigned t = 0; t < threads; t++)
      pthread_join(ids[t], NULL);
  }
  for (unsigned t = 0; t < threads; t++)
    sum += sums[t];
  return sum;
}
static void unload(void) {
  for (uint32_t i = 0; i < count; i++)
    free(files[i].data);
  free(files);
}
int main(int argc, char **argv) {
  if (argc != 4)
    return 2;
  unsigned threads = atoi(argv[3]);
  if (!threads || threads > 128)
    return 2;
  folding = strcmp(argv[2], "fold") == 0;
  load(argv[1]);
  uint32_t warm = run(threads);
  unload();
  printf("%u\n", warm);
  uint64_t t0 = stamp();
  load(argv[1]);
  uint64_t t1 = stamp();
  uint32_t sum = run(threads);
  uint64_t t2 = stamp();
  printf("%u\n%llu\n%llu\n", sum, (unsigned long long)(t1 - t0),
         (unsigned long long)(t2 - t1));
  unload();
}
