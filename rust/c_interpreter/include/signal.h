#ifndef __CODEX_SIGNAL_H
#define __CODEX_SIGNAL_H

typedef int sig_atomic_t;
typedef void __codex_sighandler_t(int);

extern void __codex_sig_dfl(int);
extern void __codex_sig_ign(int);
extern void __codex_sig_err(int);

#define SIG_DFL __codex_sig_dfl
#define SIG_IGN __codex_sig_ign
#define SIG_ERR __codex_sig_err

#define SIGABRT 6
#define SIGFPE 8
#define SIGILL 4
#define SIGINT 2
#define SIGSEGV 11
#define SIGTERM 15

__codex_sighandler_t *signal(int sig, __codex_sighandler_t *func);
int raise(int sig);

#endif
