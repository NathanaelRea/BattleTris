/* BTConfig.H.  Generated automatically by configure.  */
/* BTConfig.H.in.  Generated automatically from configure.in by autoheader.  */

/*
 * BattleTris site configuration header file
 */

#ifndef BTCONFIG_H
#define BTCONFIG_H

/* Define to `int' if <sys/types.h> doesn't define.  */
/* #undef gid_t */

/* Define if you support file names longer than 14 characters.  */
#define HAVE_LONG_FILE_NAMES 1

/* Define if you have <sys/wait.h> that is POSIX.1 compatible.  */
#define HAVE_SYS_WAIT_H 1

/* Define to `int' if <sys/types.h> doesn't define.  */
/* #undef mode_t */

/* Define to `long' if <sys/types.h> doesn't define.  */
/* #undef off_t */

/* Define to `int' if <sys/types.h> doesn't define.  */
/* #undef pid_t */

/* Define if you need to in order for stat and other things to work.  */
/* #undef _POSIX_SOURCE */

/* Define as the return type of signal handlers (int or void).  */
#define RETSIGTYPE int

/* Define to `unsigned' if <sys/types.h> doesn't define.  */
/* #undef size_t */

/* Define if the `S_IS*' macros in <sys/stat.h> do not work properly.  */
/* #undef STAT_MACROS_BROKEN */

/* Define if you have the ANSI C header files.  */
#define STDC_HEADERS 1

/* Define if you can safely include both <sys/time.h> and <time.h>.  */
#define TIME_WITH_SYS_TIME 1

/* Define to `int' if <sys/types.h> doesn't define.  */
/* #undef uid_t */

/*
 * Defined automatically if --with-sun-audio is passed to configure to
 * enable the Sun audio code, which needs the libraries normally found
 * in /usr/demo/SOUND on Sun machines, or if configure finds /usr/demo/SOUND
 * on its own.
 */
#ifdef __sun
#define HAVE_SUNAUDIO 1
#else
#define HAVE_SUNAUDIO 0
#endif

/*
 * Define the type of file descriptor set arguments to select.  The configure
 * script will try to determine this and attempt to compile a test program to
 * verify that it works.
 */
#define SELECTARGTYPE fd_set

/*
 * Define a value to return from our master signal handling function which is
 * appropriate for the type of RETSIGTYPE.  By default we define RETSIGVAL as
 * an empty expansion if RETSIGTYPE is void.
 */
#define RETSIGVAL ((RETSIGTYPE) 0)

/* The number of bytes in a int.  */
#define SIZEOF_INT 4

/* Define if you have the memcpy function.  */
#define HAVE_MEMCPY 1

/* Define if you have the select function.  */
#define HAVE_SELECT 1

/* Define if you have the setsid function.  */
#define HAVE_SETSID 1

/* Define if you have the sigaction function.  */
#define HAVE_SIGACTION 1

/* Define if you have the sigprocmask function.  */
#define HAVE_SIGPROCMASK 1

/* Define if you have the strchr function.  */
#define HAVE_STRCHR 1

/* Define if you have the uname function.  */
#define HAVE_UNAME 1

/* Define if you have the <X11/extensions/shape.h> header file.  */
#define HAVE_X11_EXTENSIONS_SHAPE_H 1

/* Define if you have the <dirent.h> header file.  */
#define HAVE_DIRENT_H 1

/* Define if you have the <ndir.h> header file.  */
/* #undef HAVE_NDIR_H */

/* Define if you have the <stropts.h> header file.  */
/* #undef HAVE_STROPTS_H */

/* Define if you have the <sys/dir.h> header file.  */
/* #undef HAVE_SYS_DIR_H */

/* Define if you have the <sys/ndir.h> header file.  */
/* #undef HAVE_SYS_NDIR_H */

/* Define if you have the <sys/select.h> header file.  */
#define HAVE_SYS_SELECT_H 1

/* Define if you have the <sys/time.h> header file.  */
#define HAVE_SYS_TIME_H 1

/* Define if you have the <sys/types.h> header file.  */
#define HAVE_SYS_TYPES_H 1

/* Define if you have the <unistd.h> header file.  */
#define HAVE_UNISTD_H 1

/* Make sure we have all the types we need */

#if HAVE_SYS_TYPES_H
# include <sys/types.h>
#endif

#if TIME_WITH_SYS_TIME
# include <sys/time.h>
# include <time.h>
#else
# if HAVE_SYS_TIME_H
#  include <sys/time.h>
# else
#  include <time.h>
# endif
#endif

#if HAVE_SYS_SELECT_H
# include <sys/select.h>
#endif

/*
 * Make sure we have FD_SET macros for calling select.  We assume that
 * configure has already defined SELECTARGTYPE to be the proper argument type
 * for using select, and that AC_CHECK_SIZEOF(int) has been called.
 */

#ifdef FD_SET
# define SELECTSETSIZE FD_SETSIZE
#else
# define SELECT_SET_SIZE SIZEOF_INT
# define FD_SET(n, p) (*(p) |= (1 << (n)))
# define FD_CLR(n, p) (*(p) &= ~(1 << (n)))
# define FD_ISSET(n, p) (*(p) & (1 << (n)))
# define FD_ZERO(p) (*(p) = 0)
#endif

/* Make sure we have access to string and memory functions */

#if defined(STDC_HEADERS) || defined(HAVE_STRING_H)
# include <string.h>
# if !defined(STDC_HEADERS) && HAVE_MEMORY_H
#  include <memory.h>
# endif
# ifndef index
#  define index strchr
# endif
# ifndef rindex
#  define rindex strrchr
# endif
# ifndef bcopy
#  define bcopy(s, d, n) memcpy ((d), (s), (n))
# endif
# ifndef bcmp
#  define bcmp(s1, s2, n) memcmp ((s1), (s2), (n))
# endif
# ifndef bzero
#  define bzero(s, n) memset ((s), 0, (n))
# endif
#else
# include <strings.h>
#endif

/* socklen_t is available on macOS and Solaris */
#if defined(__sun) || defined(__APPLE__)
#define HAVE_SOCKLEN_T
#endif

/* _XPG4_2: use msg_control/msg_controllen for fd passing (POSIX cmsg) */
#if !defined(__sun) || defined(__APPLE__)
#define _XPG4_2
#endif

/* drand48/lrand48/uint_t: macOS provides drand48/lrand48 natively */
#if defined(__sun) && !defined(__APPLE__)
#define drand48()	((double)rand() / (double)RAND_MAX)
#define lrand48()	((long)rand())
#define srand48(x)	srand(x)
#endif

/* uint_t is a Solaris type not in standard headers on macOS */
#if !defined(__sun)
typedef unsigned int	uint_t;
#endif

#endif
