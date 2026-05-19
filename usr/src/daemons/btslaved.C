/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: btslaved.C                                          */
/*    DATE: Sun Apr 17 01:30:57 1994                            */
/****************************************************************/

#include "BTConfig.H"


#include <iostream>
using namespace std;
#include <stdio.h>

#if STDC_HEADERS
# include <stdlib.h>
#endif

#if HAVE_UNISTD_H
# include <unistd.h>
#endif

#include "StreamSocketErr.H"

#include "BTDBErr.H"
#include "BTDirs.H"
#include "BTConfigFile.H"
#include "BTSlave.H"

BTConfigFile *g_conf;				// Configuration file object

static char *configfile = "btserver.cf";	// Configuration file pathname
static int prindex = -1;			// Daemon process index

void usage(char *pname)
{
  cout << "Usage: " << pname << " -h? | -i index\n";
  cout << "       -h, -?         Display this usage information.\n";
  cout << "       -i index       Specify the slave number for this process.\n";
  cout << "       -f configfile  Specify the configuration file pathname.\n";
  cout << flush;
}

void parse_args(int argc, char **argv)
{
  extern int optind;
  extern char *optarg;
  int c;

  while(optind < argc) {
    while((c = getopt(argc, argv, "h?i:f:")) != (int) EOF) {
      switch(c) {

      case '?':
      case 'h':
	usage(argv[0]);
	exit(0);

      case 'i':
	prindex = atoi(optarg);
	break;

      case 'f':
        configfile = optarg;
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

  if(prindex < 0) {
    cerr << argv[0] << ": \"-i\" argument is mandatory" << endl;
    usage(argv[0]);
    exit(2);
  }
}

int main(int argc, char *argv[])
{
  char logpath[1024];
  short err;

  parse_args(argc, argv);

  if((g_conf = new BTConfigFile(configfile)) == 0) {
    cerr << argv[0] << ": Failed to allocate needed memory" << endl;
    return 1; 
  }

  if(g_conf->status() != BTCONFIGFILE_OK) {
    delete g_conf;
    return 1;
  }

  sprintf(logpath, "%s/%s%ld.log", g_conf->logsdir(), BTSD_LOGFILE, getpid());
  BTSlave slave(logpath, prindex);

  if((err = slave.err()) < 0) {
    cerr << argv[0] << ": Fatal error occurred: "
         << StreamSocketErrMsg(err) << endl;
    delete g_conf;
    return 1;
  }

  if((err = slave.run()) < 0) {
    cerr << argv[0] << ": Fatal error occurred: "
         << BTDBErrMsg(err) << endl;
    delete g_conf;
    return 1;
  }

  delete g_conf;
  return 0;
}
