/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: BTServer.H                                          */
/*    DATE: Tue Sep 27 23:22:17 1994                            */
/****************************************************************/

#ifndef BTSERVER_H
#define BTSERVER_H

#include "BTConfig.H"


#include <fstream>
using namespace std;

#include "BTConstants.H"
#include "PacketBuffer.H"

class StreamSocket;
class BTMDSlave;

class BTServer {

friend class BTMDTermHandler;
friend class BTMDListHandler;
friend class BTMDSuspHandler;
friend class BTMDChldHandler;
friend class BTMDHangHandler;

private:
  int death_;
  int sidx_;

  StreamSocket **sockets_;
  BTMDSlave **slaves_;
  int nslaves_;

  StreamSocket *listener_;
  PacketBuffer pbuf_;

  ofstream *log_;
  char *pidfile_;

  int enabled_;
  short err_;

private:
  void toggleListener();
  void restartSlaves();
  void listSlaves();
  void killSlaves();

  void spawnSlave(int prindex);
  void deadSlave();

  int acceptClient(StreamSocket *client);

public:
  BTServer(int nslaves, int port);
  ~BTServer();

  short run();

private:
  BTServer(const BTServer& other);
  BTServer& operator=(const BTServer& other);
};

#endif
