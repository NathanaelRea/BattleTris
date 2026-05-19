/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: BTNetworkEntry.C                                    */
/*    DATE: Thu Apr 21 16:23:54 1994                            */
/****************************************************************/

#include "BTConfig.H"

#if HAVE_UNISTD_H
# include <unistd.h>
#endif

#include <assert.h>
#include <stdio.h>


#include <iostream>
using namespace std;

#include <iomanip>
using namespace std;

#include "BTNetworkEntry.H"
#include "BTProtocol.H"
#include "BTNetwork.H"
#include "BTDB.H"
#include "BTDBErr.H"

BTNetworkEntry::BTNetworkEntry()
: BTDBRecord(""), timestamp_(0), pid_(0), addrnet_(0), addrlna_(0),
  port_(0), maxwpn_(0), majver_(0), minver_(0), status_(BTSTATUS_UNKNOWN)
{
  bzero((char *) userName_, sizeof(userName_));
  bzero((char *) hostName_, sizeof(hostName_));
  size_ += datalen();
  valid_ = 0;
}

BTNetworkEntry::BTNetworkEntry(char *userName, char *hostName, InetAddress& addr)
: BTDBRecord(userName), timestamp_(time(0)), pid_(getpid()),
  addrnet_(addr.net()), addrlna_(addr.lna()), port_(addr.port()),
  maxwpn_(BT_MAX_WEAPONS), majver_(BT_MAJOR_VER), minver_(BT_MINOR_VER),
  status_(BTSTATUS_WAITING)
{
  assert(userName != 0);
  assert(hostName != 0);

  sprintf(key(), "%-.*s%-.*s%-d", BT_USERNAMELEN,
          userName, BT_HOSTABBRLEN, hostName, port_);

  bzero((char *) userName_, sizeof(userName_));
  bzero((char *) hostName_, sizeof(hostName_));

  strncpy(userName_, userName, BT_USERNAMELEN);
  strncpy(hostName_, hostName, BT_HOSTNAMELEN);

  size_ += datalen();
  valid_ = 1;
}

short BTNetworkEntry::read(int fd, off_t offset, off_t nbytes)
{
  if(lseek(fd, offset, SEEK_SET) < 0)
    return ERRBTDBSEEK;

  char *buf = new char [nbytes];

  if(::read(fd, (void *) buf, nbytes) != nbytes) {
    delete [] buf;
    return ERRBTDBREAD;
  }

  (void) BTNetworkEntry::readbuf(buf);
  delete [] buf;
  return ERRBTDBNOERR;
}

short BTNetworkEntry::write(int fd, off_t offset)
{
  if(lseek(fd, offset, SEEK_SET) < 0)
    return ERRBTDBSEEK;

  char *buf = new char [size_];

  (void) BTNetworkEntry::writebuf(buf);

  if(writen(fd, (void *) buf, size_) != size_) {
    delete [] buf;
    return ERRBTDBWRITE;
  }

  delete [] buf;
  return ERRBTDBNOERR;
}

char *BTNetworkEntry::writebuf(char *bufptr)
{
  unsigned short ts;
  unsigned long tl;

  bcopy((char *) key_, (char *) bufptr, sizeof(key_));
  bufptr += sizeof(key_);
  bcopy((char *) userName_, (char *) bufptr, sizeof(userName_));
  bufptr += sizeof(userName_);
  bcopy((char *) hostName_, (char *) bufptr, sizeof(hostName_));
  bufptr += sizeof(hostName_);

  BTNET_PUTLONG(bufptr, tl, timestamp_);
  BTNET_PUTLONG(bufptr, tl, pid_);
  BTNET_PUTLONG(bufptr, tl, addrnet_);
  BTNET_PUTLONG(bufptr, tl, addrlna_);
  BTNET_PUTSHORT(bufptr, ts, port_);
  BTNET_PUTSHORT(bufptr, ts, maxwpn_);
  BTNET_PUTSHORT(bufptr, ts, majver_);
  BTNET_PUTSHORT(bufptr, ts, minver_);
  BTNET_PUTSHORT(bufptr, ts, status_);

  return bufptr;
}

char *BTNetworkEntry::readbuf(char *bufptr)
{
  char *bufstart = bufptr;
  unsigned short ts;
  unsigned long tl;

  bcopy((char *) bufptr, (char *) key_, sizeof(key_));
  bufptr += sizeof(key_);
  bcopy((char *) bufptr, (char *) userName_, sizeof(userName_));
  bufptr += sizeof(userName_);
  bcopy((char *) bufptr, (char *) hostName_, sizeof(hostName_));
  bufptr += sizeof(hostName_);

  BTNET_GETLONG(bufptr, tl, timestamp_);
  BTNET_GETLONG(bufptr, tl, pid_);
  BTNET_GETLONG(bufptr, tl, addrnet_);
  BTNET_GETLONG(bufptr, tl, addrlna_);
  BTNET_GETSHORT(bufptr, ts, port_);
  BTNET_GETSHORT(bufptr, ts, maxwpn_);
  BTNET_GETSHORT(bufptr, ts, majver_);
  BTNET_GETSHORT(bufptr, ts, minver_);
  BTNET_GETSHORT(bufptr, ts, status_);

  size_ = bufptr - bufstart;
  valid_ = 1;

  return bufptr;
}

int BTNetworkEntry::operator==(const BTNetworkEntry& other)
{
  return (strcmp(userName_, other.userName_) == 0) &&
    (strcmp(hostName_, other.hostName_) == 0) &&
    (timestamp_ == other.timestamp_) &&
    (pid_ == other.pid_) &&
    (addrnet_ == other.addrnet_) &&
    (addrlna_ == other.addrlna_) &&
    (port_ == other.port_) &&
    (maxwpn_ == other.maxwpn_) &&
    (majver_ == other.majver_) &&
    (minver_ == other.minver_) &&
    (status_ == other.status_);
}

int BTNetworkEntry::operator!=(const BTNetworkEntry& other)
{
  return (strcmp(userName_, other.userName_) != 0) ||
    (strcmp(hostName_, other.hostName_) != 0) ||
    (timestamp_ != other.timestamp_) ||
    (pid_ != other.pid_) ||
    (addrnet_ != other.addrnet_) ||
    (addrlna_ != other.addrlna_) ||
    (port_ != other.port_) ||
    (maxwpn_ != other.maxwpn_) ||
    (majver_ != other.majver_) ||
    (minver_ != other.minver_) ||
    (status_ != other.status_);
}

int BTNetworkEntry::compare(const void *left, const void *right)
{
  assert(left != 0);
  assert(right != 0);

  BTNetworkEntry *lval = *((BTNetworkEntry **) left);
  BTNetworkEntry *rval = *((BTNetworkEntry **) right);

  assert(lval != 0);
  assert(rval != 0);

  int rval1 = strcmp(lval->userName_, rval->userName_);
  if(rval1 == 0) {
    int rval2 = strcmp(lval->hostName_, rval->hostName_);
    if(rval2 == 0) {
      if(lval->timestamp_ < rval->timestamp_) return -1;
      if(lval->timestamp_ > rval->timestamp_) return 1;
      if(lval->pid_ < rval->pid_) return -1;
      if(lval->pid_ > rval->pid_) return 1;
      if(lval->addrnet_ < rval->addrnet_) return -1;
      if(lval->addrnet_ > rval->addrnet_) return 1;
      if(lval->addrlna_ < rval->addrlna_) return -1;
      if(lval->addrlna_ > rval->addrlna_) return 1;
      if(lval->port_ < rval->port_) return -1;
      if(lval->port_ > rval->port_) return 1;
      if(lval->maxwpn_ < rval->maxwpn_) return -1;
      if(lval->maxwpn_ > rval->maxwpn_) return 1;
      if(lval->majver_ < rval->majver_) return -1;
      if(lval->majver_ > rval->majver_) return 1;
      if(lval->minver_ < rval->minver_) return -1;
      if(lval->minver_ > rval->minver_) return 1;
      if(lval->status_ < rval->status_) return -1;
      if(lval->status_ > rval->status_) return 1;
      return 0;
    }

    return rval2;
  }

  return rval1;
}

BTNetworkEntry::BTNetworkEntry(const BTNetworkEntry& other)
: BTDBRecord(other)
{
  strncpy(key(), other.key(), BTDBRECORD_KEYLEN);
  strncpy(userName_, other.userName_, BT_USERNAMELEN);
  strncpy(hostName_, other.hostName_, BT_HOSTNAMELEN);
  timestamp_ = other.timestamp_;
  pid_ = other.pid_;
  addrnet_ = other.addrnet_;
  addrlna_ = other.addrlna_;
  port_ = other.port_;
  maxwpn_ = other.maxwpn_;
  majver_ = other.majver_;
  minver_ = other.minver_;
  status_ = other.status_;
}

BTNetworkEntry& BTNetworkEntry::operator=(const BTNetworkEntry& other)
{
  if(this == &other)
    return *this;

  strncpy(key_, other.key_, BTDBRECORD_KEYLEN);
  valid_ = other.valid_;
  size_ = other.size_;

  strncpy(userName_, other.userName_, BT_USERNAMELEN);
  strncpy(hostName_, other.hostName_, BT_HOSTNAMELEN);
  timestamp_ = other.timestamp_;
  pid_ = other.pid_;
  addrnet_ = other.addrnet_;
  addrlna_ = other.addrlna_;
  port_ = other.port_;
  maxwpn_ = other.maxwpn_;
  majver_ = other.majver_;
  minver_ = other.minver_;
  status_ = other.status_;

  return *this;
}

ostream& operator<<(ostream& os, BTNetworkEntry& entry)
{
  os << "dbkey=<" << entry.key_ << ">\n";
  os << "valid=[" << setw(1) << entry.valid_ << "] size=["
     << entry.size() << "]\n";

  os << "stamp=[" << entry.timestamp_ << "]\n";
  os << "  pid=[" << entry.pid_ << "]\n";

  os << "usern=<" << entry.userName_ << ">\n";
  os << "hostn=<" << entry.hostName_ << ">\n";

  switch(entry.status_) {
  case BTSTATUS_UNKNOWN:
    os << " stat=UNKNOWN\n";
    break;
  case BTSTATUS_WAITING:
    os << " stat=WAITING\n";
    break;
  case BTSTATUS_PLAYING:
    os << " stat=PLAYING\n";
    break;
  default:
    os << "*stat=[" << entry.status_ << "]\n";
  }

  os << " addr: net=[" << entry.addrnet_ << "] lna=[" << entry.addrlna_
     << "] prt=[" << entry.port_ << "]\n";

  os << "versn: maj=[" << entry.majver_ << "] min=[" << entry.minver_
     << "] wpn=[" << entry.maxwpn_ << "]\n";
    
  return os;
}
