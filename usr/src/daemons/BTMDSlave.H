/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: BTMDSlave.H                                         */
/*    DATE: Mon Oct  3 00:44:02 1994                            */
/****************************************************************/

#ifndef BTMDSLAVE_H
#define BTMDSLAVE_H

#include "BTConfig.H"

#if HAVE_UNISTD_H
# include <unistd.h>
#endif


#include <iostream>
using namespace std;

#include "PacketBuffer.H"

class StreamSocket;

class BTMDSlave {

friend ostream& operator<<(ostream& os, BTMDSlave& slave);

private:
  PacketBuffer pbuf_;
  StreamSocket *slaveSock_;
  timeval timeout_;
  int prindex_;
  pid_t pid_;
  short err_;

public:
  BTMDSlave(StreamSocket *sock, int prindex = 0);
  ~BTMDSlave();

  short acceptClient(StreamSocket *client);

  short error() { return err_; }
  pid_t pid() { return pid_; }

private:
  BTMDSlave(const BTMDSlave& other);
  BTMDSlave& operator=(const BTMDSlave& other);
};

#endif
