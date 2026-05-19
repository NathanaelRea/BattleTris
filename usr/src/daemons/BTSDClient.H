/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: BTSDClient.H                                        */
/*    DATE: Wed Oct  5 14:00:42 1994                            */
/****************************************************************/

#ifndef BTSDCLIENT_H
#define BTSDCLIENT_H

#include "BTConfig.H"


#include <iostream>
using namespace std;

#include "StreamSocket.H"
#include "PacketBuffer.H"

#include "BTConstants.H"
#include "BTProtocol.H"
#include "BTNetworkEntry.H"

class BTSDClient {

friend inline ostream& operator<<(ostream& os, BTSDClient& client);

private:
  BTNetworkEntry entry_;
  PacketBuffer pbuf_;
  StreamSocket *socket_;
  short err_;

public:
  BTSDClient(StreamSocket *socket);
  ~BTSDClient();

  BTNetworkEntry *entry() { return &entry_; }
  StreamSocket *sock() { return socket_; }
  int sockfd() { return socket_->sock(); }
  short error() { return err_; }

private:
  BTSDClient(const BTSDClient& other);
  BTSDClient& operator=(const BTSDClient& other);
};

ostream& operator<<(ostream& os, BTSDClient& client)
{
  os << client.entry_.userName_ << '@' << client.entry_.hostName_
     << '.' << client.entry_.port_;

  return os;
}

#endif
