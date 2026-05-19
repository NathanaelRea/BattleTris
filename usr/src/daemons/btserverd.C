/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: btserverd.C                                         */
/*    DATE: Tue Sep 27 10:10:57 1994                            */
/****************************************************************/

#include "BTConfig.H"

#if HAVE_UNISTD_H
# include <unistd.h>
#endif

#include <sys/stat.h>

#if STDC_HEADERS
# include <stdlib.h>
#endif


#include <iostream>
using namespace std;

#include "StreamSocketErr.H"

#include "BTDirs.H"
#include "BTConfigFile.H"
#include "BTServer.H"
#include "BTProtocol.H"

BTConfigFile *g_conf;				// Configuration file object
char *configfile = "btserver.cf";		// Default config file path

static int port = BT_SERVER_PORT;		// Default server port
static int nslaves = 5;				// Default number of slaves

void usage(char *pname)
{
  cout << "Usage: " << pname
       << " [-h?] | [-f configfile] [-p port] [-n nslaves]\n";
  
  cout << "       -h, -?         Display this usage information.\n";
  cout << "       -f configfile  Specify the pathname of the configuration\n";
  cout << "                      file which contains pathnames of files and\n";
  cout << "                      directories used by the server.\n";
  cout << "       -p port        Specify the port number on which the\n";
  cout << "                      server daemon should listen for incoming\n";
  cout << "                      connection requests.  If no -p flag is\n";
  cout << "                      specified, the server listens on port\n";
  cout << "                      " << BT_SERVER_PORT << " by default.\n";
  cout << "       -n nslaves     Spawn nslaves slave daemons.  If no\n";
  cout << "                      -n flag is specified, 5 daemons are\n";
  cout << "                      spawned by default.\n";

  cout << flush;
}

void parse_args(int argc, char **argv)
{
  extern int optind;
  extern char *optarg;
  int c;

  while(optind < argc) {
    while((c = getopt(argc, argv, "h?f:p:n:")) != (int) EOF) {
      switch(c) {

      case '?':
      case 'h':
	usage(argv[0]);
	exit(0);

      case 'f':
	configfile = optarg;
	break;

      case 'p':
        if((*optarg < '0') || (*optarg > '9')) {
          cerr << argv[0] << ": Invalid port number specified" << endl;
          usage(argv[0]);
          exit(2);
        }

        port = atoi(optarg);
        break;

      case 'n':
	nslaves = atoi(optarg);
	break;

      default:
	usage(argv[0]);
	exit(2);
      }
    }
    
    if(optind < argc) {
      usage(argv[0]);
      exit(2);
    }
  }
}

int main(int argc, char *argv[])
{
  parse_args(argc, argv);

  if((g_conf = new BTConfigFile(configfile)) == 0) {
    cerr << argv[0] << ": Failed to allocate needed memory" << endl;
    return 1;
  }

  if(g_conf->status() != BTCONFIGFILE_OK) {
    delete g_conf;
    return 1;
  }

  switch(fork()) {

  case -1:
    cerr << argv[0] << ": Failed to fork child process" << endl;
    delete g_conf;
    return 1;

  case 0:
#if HAVE_SETSID
    setsid();
#endif
    umask(0);
    break;

  default:
    delete g_conf;
    return 0;
  }

  BTServer server(nslaves, port);

  if(server.run() < 0) {
    cerr << argv[0] << ": Fatal error occurred" << endl;
    delete g_conf;
    return 1;
  }

  delete g_conf;
  return 0;
}
