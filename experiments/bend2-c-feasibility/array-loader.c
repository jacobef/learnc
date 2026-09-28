// Same corpus loader as loader.c, but source text uses owned Array<U32>.
// Kept as a separate representation experiment; token IR remains a list.
static FILE *input;
static uint32_t read_u32(void) {
  unsigned char b[4];
  if (fread(b, 1, 4, input) != 4)
    exit(2);
  return b[0] | (uint32_t)b[1] << 8 | (uint32_t)b[2] << 16 |
         (uint32_t)b[3] << 24;
}
static Term load_tree(Env e, uint32_t n) {
  if (n == 1) {
    uint32_t len = read_u32(), depth = 0;
    while ((1ull << depth) < len)
      depth++;
    unsigned char *bytes = malloc(len);
    if (!bytes || fread(bytes, 1, len, input) != len)
      exit(2);
    Term zero = 0;
    Term a = blk_new(e, 0, depth, 0, 1, &zero);
    for (uint32_t i = 0; i < len; i++)
      blk_write(e.mem, 0, term_loc(a), i, bytes[i]);
    free(bytes);
    return io_node(e, CID_SOURCE, a, len, 0);
  }
  Term l = load_tree(e, n / 2), r = load_tree(e, n - n / 2);
  return io_node(e, CID_BRANCH, l, r, 0);
}
Term load_run(Env e, Term *f, IoWork *w) {
  const char *path = getenv("CORPUS");
  input = path ? fopen(path, "rb") : NULL;
  if (!input) {
    perror("CORPUS");
    exit(2);
  }
  uint32_t n = read_u32();
  if (!n)
    exit(2);
  Term t = load_tree(e, n);
  fclose(input);
  return t;
}
Term stamp_run(Env e, Term *f, IoWork *w) { return io_tick() / 1000; }
static void __attribute__((constructor)) bench_register(void) {
  io_eff(CID_LOAD, load_run, 0);
  io_eff(CID_STAMP, stamp_run, 0);
}
