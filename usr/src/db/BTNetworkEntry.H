/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: BTNetworkEntry.H                                    */
/*    DATE: Thu Apr 21 16:16:28 1994                            */
/****************************************************************/

#ifndef BTNETWORKENTRY_H
#define BTNETWORKENTRY_H

#include "BTConfig.H"


#include <iostream>
using namespace std;

#include "Address.H"
#include "BTDBRecord.H"
#include "BTConstants.H"

enum BTNetworkStatus
{
  BTSTATUS_UNKNOWN,
  BTSTATUS_WAITING,
  BTSTATUS_PLAYING
};

class BTNetworkEntry : public BTDBRecord {

friend ostream& operator<<(ostream& os, BTNetworkEntry& entry);

public:
  char userName_[BT_USERNAMELEN + 1];
  char hostName_[BT_HOSTNAMELEN + 1];
  unsigned long timestamp_;
  unsigned long pid_;
  unsigned long addrnet_;
  unsigned long addrlna_;
  unsigned short port_;
  unsigned short maxwpn_;
  unsigned short majver_;
  unsigned short minver_;
  unsigned short status_;

public:
  BTNetworkEntry();
  BTNetworkEntry(char *userName, char *hostName, InetAddress& addr);
  ~BTNetworkEntry() {}

  short read(int fd, off_t offset, off_t nbytes);
  short write(int fd, off_t offset);

  char *writebuf(char *bufptr);
  char *readbuf(char *bufptr);

  size_t datalen() {
    return sizeof(userName_) + sizeof(hostName_) +
      sizeof(timestamp_) + sizeof(pid_) +
      sizeof(addrnet_) + sizeof(addrlna_) + sizeof(port_) +
      sizeof(maxwpn_) + sizeof(majver_) + sizeof(minver_) + sizeof(status_);
  }

  void touch() { timestamp_ = time(0); }

  int operator==(const BTNetworkEntry& other);
  int operator!=(const BTNetworkEntry& other);

  static int compare(const void *left, const void *right);

  BTNetworkEntry(const BTNetworkEntry& other);
  BTNetworkEntry& operator=(const BTNetworkEntry& other);
};

#endif
