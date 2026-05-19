/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: BTSlave.H                                           */
/*    DATE: Wed Oct  5 01:04:50 1994                            */
/****************************************************************/

#ifndef BTSLAVE_H
#define BTSLAVE_H

#include "BTConfig.H"


#include <fstream>
using namespace std;

#include "PacketBuffer.H"

class StreamSocket;
class BTDBServer;
class BTSDClient;

enum {
  BTSD_MAXCLIENTS = 10,

  BTSD_CTMOUT_SECS = 60,
  BTSD_CTMOUT_USECS = 0,

  BTSD_TMOUT_SECS = 30,
  BTSD_TMOUT_USECS = 0,

  BTSD_POLL_SECS = 30,
  BTSD_POLL_USECS = 0
};

class BTSlave {

friend class BTSDTermHandler;
friend class BTSDLst1Handler;
friend class BTSDLst2Handler;
friend class BTSDPipeHandler;
friend class BTSDHangHandler;

private:
  StreamSocket *master_;
  PacketBuffer pbuf_;
  PacketBuffer cbuf_;

  int maxfd_;
  int awol_;

  BTSDClient **clients_;
  int maxclients_;

  BTDBServer *dbserver_;
  ofstream log_;

  timeval timeout_;
  short err_;

private:
  void process(int idx);

  void listClients();
  void listQueue();
  void restart();

public:
  BTSlave(const char *logpath, int prindex, int maxclients = BTSD_MAXCLIENTS);
  ~BTSlave() { BTSlave::terminate(); }

  void disconnect();
  void terminate();

  short obey();
  short run();

  short err() { return err_; }

private:
  BTSlave(const BTSlave& other);
  BTSlave& operator=(const BTSlave& other);
};

#endif
