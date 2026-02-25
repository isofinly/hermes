#ifndef HERMES_MASSCAN_WRAPPER_H
#define HERMES_MASSCAN_WRAPPER_H

// This wrapper intentionally includes only masscan's public header to keep
// bindgen input deterministic.
#include <signal.h>
#define stack_t masscan_stack_t
#include "../masscan/src/masscan.h"
#undef stack_t

int masscan_cli_main(int argc, char **argv);

#endif
