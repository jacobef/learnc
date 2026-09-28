// Benchmark scaffolding only: deserialize actual source bytes into Bend lists.
// No lexing/parsing/folding is performed in C for the Bend measurements.
static FILE *input;
static uint32_t read_u32(void) {
  unsigned char b[4];
  if (fread(b, 1, 4, input) != 4) {
    fprintf(stderr, "truncated corpus\n");
    exit(2);
  }
  return b[0] | (uint32_t)b[1] << 8 | (uint32_t)b[2] << 16 |
         (uint32_t)b[3] << 24;
}
static Term load_tree(Env e, uint32_t n) {
  if (n == 1) {
    uint32_t len = read_u32();
    unsigned char *bytes = malloc(len);
    if (!bytes || fread(bytes, 1, len, input) != len)
      exit(2);
    Term xs = term_pak(CID_NIL, 0);
    for (uint32_t i = len; i; --i)
      xs = io_node(e, CID_CON, bytes[i - 1], xs, 0);
    free(bytes);
    return io_box(e, CID_SOURCE, xs, 0);
  }
  Term l = load_tree(e, n / 2);
  Term r = load_tree(e, n - n / 2);
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
  Term tree = load_tree(e, n);
  fclose(input);
  return tree;
}
Term stamp_run(Env e, Term *f, IoWork *w) { return io_tick() / 1000; }
static void __attribute__((constructor)) bench_register(void) {
  io_eff(CID_LOAD, load_run, 0);
  io_eff(CID_STAMP, stamp_run, 0);
}
