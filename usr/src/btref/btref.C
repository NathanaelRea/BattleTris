#include "BTConfig.H"


#include <iostream>
using namespace std;

#include <iomanip>
using namespace std;
#include <stdio.h>

#if HAVE_UNAME
# include <sys/utsname.h>
#endif

#if HAVE_UNISTD_H
# include <unistd.h>
#endif

#if STDC_HEADERS
# include <stdlib.h>
#endif

#include "SigReceiver.H"
#include "BTDB.H"
#include "BTDBErr.H"
#include "BTDirs.H"
#include "BTProtocol.H"
#include "BTConstants.H"
#include "BTConfigFile.H"

volatile int maskint = 0;
int prompt = 0;

#include "btcmds.H"
#include "btcmdtab.H"
#include "btsignals.H"
#include "btref.H"

static const char *BTREF_CONFIGFILE = "btserver.cf";
static const char *BTREF_CMDDELIMS = " \t\n\r";
static const int BTREF_CMDBUFLEN = 255;

BTConfigFile *g_conf = 0;

BTDB *netdb = 0;
BTDB *plydb = 0;

static void parse_args(int argc, char **argv);
static void usage(char *pname);

static Command *findcommand(register char *name);
static void tokenize(char *buf, int& argc, char **argv);

void cleanup();

int main(int argc, char *argv[])
{
  parse_args(argc, argv);

  char srvrhost[BT_HOSTNAMELEN + 1];
  char pathbuf[1024];
  char *ptr;

  if((g_conf = new BTConfigFile(BTREF_CONFIGFILE)) == 0) {
    cerr << "btref: Insufficient memory to load" << endl;
    return BTREF_ERROR;
  }

  if(!(*g_conf)) {
    delete g_conf;
    return BTREF_ERROR;
  }

#if HAVE_UNAME
  struct utsname hostinfo;
  uname(&hostinfo);
  char *thishost = hostinfo.nodename;
#else
  char thishost[257];
  gethostname(thishost, 257);
#endif

  strncpy(srvrhost, BT_SERVER_HOST, BT_HOSTNAMELEN);
  if((ptr = strchr(srvrhost, '.')) != NULL)
    *ptr = '\0';

  if(strcmp(thishost, srvrhost) != 0) {
    cerr << "btref: You must run btref from a BattleTris Server" << endl;
    cerr << "btref: This referee is compiled for " << BT_SERVER_HOST << endl;
    return BTREF_ERROR;
  }

  strcpy(pathbuf, g_conf->datadir());
  strcat(pathbuf, "/");
  strcat(pathbuf, BTDB_NETWORK);

  netdb = new BTDB(pathbuf, O_CREAT | O_RDWR);

  if(!(*netdb)) {
    cerr << "btref: Failed to open network database" << endl;
    cerr << "btref: " << BTDBErrMsg(netdb->error()) << endl;
    return BTREF_ERROR;
  }

  strcpy(pathbuf, g_conf->datadir());
  strcat(pathbuf, "/");
  strcat(pathbuf, BTDB_PLAYERS);
    
  plydb = new BTDB(pathbuf, O_CREAT | O_RDWR);

  if(!(*plydb)) {
    cerr << "btref: Failed to open player database" << endl;
    cerr << "btref: " << BTDBErrMsg(netdb->error()) << endl;
    return BTREF_ERROR;
  }

  if(atexit(cleanup)) {
    cerr << "btref: Failed to install exit handler" << endl;
    return BTREF_ERROR;
  }

  SigReceiver receiver;
  receiver.reset();

  BTRefTermHandler termHdl;

  receiver.install(SIGTERM, &termHdl);
  receiver.install(SIGINT, &termHdl);
  receiver.install(SIGHUP, &termHdl);
  receiver.install(SIGQUIT, &termHdl);

  prompt = isatty(fileno(stdin));

  if(prompt)
    cout << "BattleTris Referee Software v" << BTREF_VERSION << endl;

  register Command *c;
  char cmdbuf[BTREF_CMDBUFLEN + 1];
  char *cmdargv[BTREF_CMDBUFLEN / 2];
  int cmdargc;

  for (;;) {
    if(prompt)
      cout << "btref> " << flush;

    cin.getline(cmdbuf, BTREF_CMDBUFLEN);

    if(cin.fail() || cin.eof()) {
      cout << endl;
      break;
    }

    tokenize(cmdbuf, cmdargc, cmdargv);

    if(cmdargv[0] == 0)
      continue;

    c = findcommand(cmdargv[0]);

    if(c == (Command *) -1) {
      cerr << "btref: Ambiguous command" << endl;
      continue;
    }

    if (c == 0) {
      cerr << "btref: Invalid command" << endl;
      continue;
    }

    maskint = 1;
    (*c->handler_)(cmdargc, cmdargv);
    maskint = 0;
   }
   
   return BTREF_SUCCESS;
}

static void parse_args(int argc, char **argv)
{
  extern int optind;
  extern char *optarg;
  int c;

  while(optind < argc) {
    while((c = getopt(argc, argv, "h?f:")) != (int) EOF) {
      switch(c) {

      case '?':
      case 'h':
	usage(argv[0]);
	exit(BTREF_SUCCESS);

      case 'f':
        BTREF_CONFIGFILE = optarg;
        break;

      default:
	usage(argv[0]);
	exit(BTREF_USAGE);
      }
    }

    if(optind < argc) {
      usage(argv[0]);
      exit(BTREF_USAGE);
    }
  }
}

static void usage(char *pname)
{
  cout << "Usage: " << pname << " [-h|-?] [-f configfile]\n";
  cout << "       -h, -?         Display usage information.\n";
  cout << "                      Make sure to run btref from a valid\n";
  cout << "                      BattleTris Server host.  Type \"help\"\n";
  cout << "                      at the prompt for a list of commands.\n";
  cout << "       -f configfile  Specify the pathname of the configuration\n";
  cout << "                      file which contains pathnames of files and\n";
  cout << "                      directories used by the server.\n";
  cout << flush;
}

static Command *findcommand(register char *name)
{
  register Command *c, *found = 0;
  register int nmatches = 0;
  register int longest = 0;
  register char *p, *q;

  for(c = cmdtab; p = c->name_; c++) {
    for(q = name; *q == *p++; q++) {
      if(*q == 0)
	return(c);
    }

    if(!*q) {
      if(q - name > longest) {
	longest = q - name;
	nmatches = 1;
	found = c;
      } else if(q - name == longest)
	nmatches++;
    }
  }

  if (nmatches > 1)
    return((Command *) -1);

  return(found);
}

static void tokenize(char *buf, int& argc, char **argv)
{
  register char *tok;
  argc = 0;

  for(tok = strtok(buf, BTREF_CMDDELIMS); tok != NULL;
      tok = strtok(NULL, BTREF_CMDDELIMS))
    argv[argc++] = tok;

  argv[argc] = (char *) NULL;
}

void cmd_help(int argc, char **argv)
{
  register Command *c;
  register char *arg;

  if(argc == 1) {
    cout << "== BattleTris Referee Help ================================\n";
    cout << " Commands may be abbreviated to shortest unique string\n";
    cout << " Commands which take pattern arguments use glob syntax\n";
    cout << "===========================================================\n";

    for(c = cmdtab; c < &cmdtab[CMDTABLEN]; c++)
      cout << setw(CMDMAXLEN) << c->name_ << " ~ " << c->help_ << '\n';
  } else {
    while(--argc > 0) {
      arg = *++argv;
      c = findcommand(arg);

      if(c == (Command *) -1)
	cerr << "btref: Command \"" << arg << "\" is ambiguous" << endl;
      else if (c == (Command *) 0)
	cerr << "btref: Command \"" << arg << "\" is invalid" << endl;
      else
	cout << setw(CMDMAXLEN) << c->name_ << " ~ " << c->help_ << '\n';
    }
  }

  cout << flush;
}

void cleanup()
{
  delete g_conf;
  delete netdb;
  delete plydb;
}
