/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: BTDBServer.H                                        */
/*    DATE: Wed Oct  5 14:24:52 1994                            */
/****************************************************************/

#ifndef BTDBSERVER_H
#define BTDBSERVER_H

#include "BTConfig.H"


#include <iostream>
using namespace std;

#include "List.H"
#include "ListIter.H"

#include "BTGameStats.H"
#include "BTNetworkEntry.H"

class BTSDClient;
class BTDB;

enum {

  BTDB_PLAYERS_CYCLE = 20,	// Amount of requests before we reread db

  BTDB_NETWORK_BUFSZ = 512000,	// Default network database buffer size
  BTDB_PLAYERS_BUFSZ = 5120000	// Default player database buffer size

};

class BTDBServer {

friend inline ostream& operator<<(ostream& os, BTDBServer& dbserver);

private:
  BTDB *netdb_;
  BTDB *plydb_;

  char *netbuf_;
  unsigned long netentries_;
  unsigned long netlen_;

  char *plybuf_;
  unsigned long plyentries_;
  unsigned long plylen_;
  int plycnt_;

  List<BTGameStats*> queue_;
  ListIter<BTGameStats*> iter_;

  int restart_;

public:
  BTDBServer();
  ~BTDBServer();

  void restart() {
    restart_ = 1;
  }

  short reload();

  short modify(BTSDClient *client);
  short enqueue(BTGameStats& stats);

  int verify(BTNetworkEntry& entry);

  short insert(BTSDClient *client);
  short revoke(BTSDClient *client);

  short processq();
  short update();

  char *netbuf() { return netbuf_; }
  unsigned long netentries() { return netentries_; }
  unsigned long netlen() { return netlen_; }

  char *plybuf() { return plybuf_; }
  unsigned long plyentries() { return plyentries_; }
  unsigned long plylen() { return plylen_; }

private:
  BTDBServer(const BTDBServer& other);
  BTDBServer& operator=(const BTDBServer& other);
};

ostream& operator<<(ostream& os, BTDBServer& dbserver)
{
  BTGameStats *stats;
  int empty = 1;

  dbserver.iter_.jump_before_head();
  while(dbserver.iter_.peek_next(stats)) {
    os << "INFO: => Results of " << stats->winnerName_ << " vs. "
       << stats->loserName_ << endl;
    dbserver.iter_.inc();
    empty = 0;
  }

  if(empty)
    os << "INFO: Queue is empty" << endl;

  return os;
}

#endif
