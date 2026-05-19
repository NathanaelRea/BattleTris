/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: StreamSocket.C                                      */
/*    DATE: Sat Apr 16 23:38:47 1994                            */
/****************************************************************/

#include "BTConfig.H"

#if STDC_HEADERS
# include <stdlib.h>
#endif

#include <sys/socket.h>
#include <sys/un.h>

#include <netinet/in.h>
#include <netinet/tcp.h>

#if HAVE_STROPTS_H && HAVE_STREAM_SOCKETS
# include <stropts.h>
#else
# include <sys/uio.h>
# include <stddef.h>
#endif

#if HAVE_UNISTD_H
# include <unistd.h>
#endif


#include <iostream>
using namespace std;
#include <assert.h>
#include <stdio.h>
#include <netdb.h>
#include <errno.h>

#include "StreamSocket.H"

#ifndef HAVE_SOCKLEN_T
typedef int socklen_t;
#endif

StreamSocket::StreamSocket(InetAddress& addr)
: Socket(socket(AF_INET, SOCK_STREAM, 0)), in_peer_(0), un_peer_(0),
  in_addr_(addr)
{
  int nodelay = 1;

  if(setsockopt(sock(), IPPROTO_TCP, TCP_NODELAY,
     (char *) &nodelay, sizeof(nodelay)) < 0)
    cerr << "StreamSocket: Warning: Failed to set TCP_NODELAY" << endl;
}

StreamSocket::StreamSocket(UnixAddress& addr)
: Socket(socket(AF_UNIX, SOCK_STREAM, 0)), in_peer_(0), un_peer_(0),
  un_addr_(addr)
{
}

StreamSocket::StreamSocket(const InetAddress& clientAddr, int clientSock,
			   const sockaddr_in& peerAddr)
: Socket(clientSock), in_peer_(new InetAddress(peerAddr)), un_peer_(0),
  in_addr_(clientAddr)
{
  int nodelay = 1;

  if(setsockopt(sock(), IPPROTO_TCP, TCP_NODELAY,
     (char *) &nodelay, sizeof(nodelay)) < 0)
    cerr << "StreamSocket: Warning: Failed to set TCP_NODELAY" << endl;
}

StreamSocket::StreamSocket(const UnixAddress& clientAddr, int clientSock,
			   const sockaddr_un& peerAddr)
: Socket(clientSock), un_peer_(new UnixAddress(peerAddr)), in_peer_(0),
  un_addr_(clientAddr)
{
}

StreamSocket::~StreamSocket()
{
  if(in_peer_)
    delete in_peer_;

  if(un_peer_)
    delete un_peer_;

  close(sock());
}

short StreamSocket::connect(InetAddress& peer)
{
  assert(in_peer_ == 0);
  in_peer_ = new InetAddress(peer);

  if(::connect(sock(), in_peer_->addr(), in_peer_->size()) < 0)
    return ERRSTREAMCONNECT;

#ifndef NDEBUG
  cout << "DEBUG: connected to port " << peer.port() << endl;
#endif

  return ERRSTREAMNOERR;
}

short StreamSocket::connect(InetAddress& peer, InetAddress& addr)
{
  assert(in_peer_ == 0);
  in_peer_ = new InetAddress(peer);

  if(::connect(sock(), in_peer_->addr(), in_peer_->size()) < 0)
    return ERRSTREAMCONNECT;

#ifndef NDEBUG
  cout << "DEBUG: connected to port " << peer.port() << endl;
#endif

  sockaddr_in name;
  socklen_t namelen = sizeof(sockaddr_in);

  if(getsockname(sock(), (sockaddr *) &name, &namelen) < 0)
    return ERRSTREAMNAME;

  addr.addr((sockaddr *) &name, namelen);
  return ERRSTREAMNOERR;
}

short StreamSocket::connect(UnixAddress& peer)
{
  assert(un_peer_ == 0);
  un_peer_ = new UnixAddress(peer);

  if(::connect(sock(), un_peer_->addr(), un_peer_->size()) < 0)
    return ERRSTREAMCONNECT;

#ifndef NDEBUG
  cout << "DEBUG: connected to UNIX domain " << peer.path() << endl;
#endif

  return ERRSTREAMNOERR;
}

short StreamSocket::connect(UnixAddress& peer, UnixAddress& addr)
{
  assert(un_peer_ == 0);
  un_peer_ = new UnixAddress(peer);

  if(::connect(sock(), un_peer_->addr(), un_peer_->size()) < 0)
    return ERRSTREAMCONNECT;

#ifndef NDEBUG
  cout << "DEBUG: connected to UNIX domain " << peer.path() << endl;
#endif

  sockaddr_un name;
  socklen_t namelen = sizeof(sockaddr_un);

  if(getsockname(sock(), (sockaddr *) &name, &namelen) < 0)
    return ERRSTREAMNAME;

  addr.addr((sockaddr *) &name, namelen);
  return ERRSTREAMNOERR;
}

short StreamSocket::listen(int backlog)
{
  if(un_addr_) {
    assert(un_peer_ == 0);

    if(::bind(sock(), un_addr_.addr(), un_addr_.size()) < 0)
      return ERRSTREAMBIND;

    if(::listen(sock(), backlog) < 0)
      return ERRSTREAMLISTEN;
  } else {
    assert(in_peer_ == 0);

    if(::bind(sock(), in_addr_.addr(), in_addr_.size()) < 0)
      return ERRSTREAMBIND;

    if(::listen(sock(), backlog) < 0)
      return ERRSTREAMLISTEN;
  }

  return ERRSTREAMNOERR;
}

short StreamSocket::listen(int backlog, InetAddress& addr)
{
  assert(in_peer_ == 0);

  if(::bind(sock(), in_addr_.addr(), in_addr_.size()) < 0)
    return ERRSTREAMBIND;

  sockaddr_in name;
  socklen_t namelen = sizeof(sockaddr_in);

  if(getsockname(sock(), (sockaddr *) &name, &namelen) < 0)
    return ERRSTREAMNAME;

  addr.addr((sockaddr *) &name, namelen);

  if(::listen(sock(), backlog) < 0)
    return ERRSTREAMLISTEN;

  return ERRSTREAMNOERR;
}

short StreamSocket::listen(int backlog, UnixAddress& addr)
{
  assert(un_peer_ == 0);

  if(::bind(sock(), un_addr_.addr(), un_addr_.size()) < 0)
    return ERRSTREAMBIND;

  sockaddr_un name;
  socklen_t namelen = sizeof(name);

  if(getsockname(sock(), (sockaddr *) &name, &namelen) < 0)
    return ERRSTREAMNAME;

  UnixAddress boundaddr(name);
  addr = boundaddr;

  if(::listen(sock(), backlog) < 0)
    return ERRSTREAMLISTEN;

  return ERRSTREAMNOERR;
}

short StreamSocket::accept(StreamSocket *& sockptr)
{
  if(un_addr_) {
    assert(un_peer_ == 0);

    sockaddr_un peerAddr;
    socklen_t peerSize = sizeof(sockaddr_un);

    int clientSock = ::accept(sock(), (sockaddr *) &peerAddr, &peerSize);

    sockaddr_un address;
    socklen_t size = sizeof(sockaddr_un);
   
    bzero((char *) &address, sizeof(sockaddr_un));

    if(getsockname(clientSock, (sockaddr *) &address, &size) < 0)
      return ERRSTREAMNAME;

    UnixAddress clientAddr(address);

#ifndef NDEBUG
    cout << "DEBUG: connection to client established at path "
	 << clientAddr.path() << endl;
#endif

    sockptr = new StreamSocket(clientAddr, clientSock, peerAddr);

  } else {

    assert(in_peer_ == 0);

    sockaddr_in peerAddr;
    socklen_t peerSize = sizeof(sockaddr_in);

    int clientSock = ::accept(sock(), (sockaddr *) &peerAddr, &peerSize);

    sockaddr_in address;
    socklen_t size = sizeof(sockaddr_in);

    bzero((char *) &address, sizeof(sockaddr_in));

    if(getsockname(clientSock, (sockaddr *) &address, &size) < 0)
      return ERRSTREAMNAME;

    InetAddress clientAddr(address);

#ifndef NDEBUG
    cout << "DEBUG: connection to client established on port "
	 << clientAddr.port() << endl;
#endif

    sockptr = new StreamSocket(clientAddr, clientSock, peerAddr);
  }

  return ERRSTREAMNOERR;
}

short StreamSocket::sendbuf(char *buf, int buflen, Address *dst)
{
  assert(dst == 0);
  assert((in_peer_ != 0) || (un_peer_ != 0));

  const char *bufptr = buf;
  int nleft = buflen;
  int nsent;

  while(nleft > 0) {
    if((nsent = ::send(sock(), bufptr, nleft, 0)) < 0) {
      if(errno == EINTR)
        continue;
      return ERRSTREAMSEND;
    }

    nleft -= nsent;
    bufptr += nsent;
  }

  return ERRSTREAMNOERR;
}

short StreamSocket::recvbuf(char *buf, int buflen, Address *src)
{
  assert(src == 0);
  assert((in_peer_ != 0) || (un_peer_ != 0));

  assert(buf != 0);
  assert(buflen > 0);

  char *bufptr = buf;
  int nleft = buflen;
  int nrecv;

  while(nleft > 0) {
    if((nrecv = ::recv(sock(), bufptr, nleft, 0)) < 0) {
      if(errno == EINTR)
        continue;
      return ERRSTREAMRECV;
    } else if(nrecv == 0) {
      return ERRSTREAMBROKEN;
    }

    nleft -= nrecv;
    bufptr += nrecv;
  }

  return ERRSTREAMNOERR;
}

short StreamSocket::peekbuf(char *buf, int buflen, Address *src)
{
  assert(src == 0);
  assert((in_peer_ != 0) || (un_peer_ != 0));

  assert(buf != 0);
  assert(buflen > 0);

  char *bufptr = buf;
  int nleft = buflen;
  int npeek;

  while(nleft > 0) {
    if((npeek = ::recv(sock(), bufptr, nleft, MSG_PEEK)) < 0) {
      if(errno == EINTR)
        continue;
      return ERRSTREAMRECV;
    } else if(npeek == 0) {
      return ERRSTREAMBROKEN;
    }

    nleft -= npeek;
    bufptr += npeek;
  }

  return ERRSTREAMNOERR;
}

short StreamSocket::recvbuf(char *buf, int buflen, timeval& delay, Address *src)
{
  assert(src == 0);
  assert((in_peer_ != 0) || (un_peer_ != 0));

  assert(buf != 0);
  assert(buflen > 0);

  SELECTARGTYPE set;
  char *bufptr = buf;
  int nleft = buflen;
  int nrecv;

  while(nleft > 0) {
    FD_ZERO(&set);
    FD_SET(sock(), &set);

    if(select(sock() + 1, (SELECTARGTYPE *) &set, (SELECTARGTYPE *) 0,
       (SELECTARGTYPE *) 0, &delay) < 0)
      return ERRSTREAMSELECT;

    if(!(FD_ISSET(sock(), &set)))
      return ERRSTREAMTIMEOUT;

    if((nrecv = ::recv(sock(), bufptr, nleft, 0)) < 0) {
      if(errno == EINTR)
        continue;
      return ERRSTREAMRECV;
    } else if(nrecv == 0) {
      return ERRSTREAMBROKEN;
    }

    nleft -= nrecv;
    bufptr += nrecv;
  }

  return ERRSTREAMNOERR;
}

short StreamSocket::peekbuf(char *buf, int buflen, timeval& delay, Address *src)
{
  assert(src == 0);
  assert((in_peer_ != 0) || (un_peer_ != 0));

  assert(buf != 0);
  assert(buflen > 0);

  SELECTARGTYPE set;
  char *bufptr = buf;
  int nleft = buflen;
  int npeek;

  while(nleft > 0) {
    FD_ZERO(&set);
    FD_SET(sock(), &set);

    if(select(sock() + 1, (SELECTARGTYPE *) &set, (SELECTARGTYPE *) 0,
       (SELECTARGTYPE *) 0, &delay) < 0)
      return ERRSTREAMSELECT;

    if(!(FD_ISSET(sock(), &set)))
      return ERRSTREAMTIMEOUT;

    if((npeek = ::recv(sock(), bufptr, nleft, MSG_PEEK)) < 0) {
      if(errno == EINTR)
        continue;
      return ERRSTREAMRECV;
    } else if(npeek == 0) {
      return ERRSTREAMBROKEN;
    }

    nleft -= npeek;
    bufptr += npeek;
  }

  return ERRSTREAMNOERR;
}

int StreamSocket::ready()
{
  timeval now;
  SELECTARGTYPE set;

  FD_ZERO(&set);
  FD_SET(sock(), &set);

  return select(sock() + 1, (SELECTARGTYPE *) &set, (SELECTARGTYPE *) 0,
                (SELECTARGTYPE *) 0, &now) > 0;
}

int StreamSocket::ready(timeval& delay)
{
  SELECTARGTYPE set;

  FD_ZERO(&set);
  FD_SET(sock(), &set);

  return select(sock() + 1, (SELECTARGTYPE *) &set, (SELECTARGTYPE *) 0,
                (SELECTARGTYPE *) 0, &delay) > 0;
}

short StreamSocket::sendfd(int fd)
{
  assert(un_peer_ != 0);
  assert(fd >= 0);

  char buf[2];		// Our own 2-byte header

  buf[0] = 0;		// Header byte 0: Flags
  buf[1] = 0;		// Header byte 1: Status (Non-zero status is an error)

  if(fd < 0)
    buf[1] = 1;		// Catch bad fd arg at runtime using header info

#if HAVE_STROPTS_H && HAVE_STREAM_SOCKETS

  if(write(sock(), buf, sizeof(buf)) != sizeof(buf))
    return ERRSTREAMSEND;

  if(fd >= 0) {
    if(ioctl(sock(), I_SENDFD, (char *) fd) < 0)
      return ERRSTREAMSEND;
  }

#else			// Either Pre-4.4BSD-based or 4.4BSD-based

  struct iovec iov[1];
  struct msghdr msg;

  iov[0].iov_base = buf;
  iov[0].iov_len = sizeof(buf);

  msg.msg_iov = iov;
  msg.msg_iovlen = 1;
  msg.msg_name = NULL;
  msg.msg_namelen = 0;

# ifdef SCM_RIGHTS	// 4.4BSD-based or SunOS 5.6+

  char cmbuf[sizeof(struct cmsghdr) + sizeof(int)];
  struct cmsghdr *cmptr = (struct cmsghdr *) cmbuf;

  if(fd < 0) {

#  ifdef _XPG4_2
    msg.msg_control = NULL;
    msg.msg_controllen = 0;
#  else
    msg.msg_accrights = NULL;
    msg.msg_accrightslen = 0;
#  endif

  } else {
    cmptr->cmsg_level = SOL_SOCKET;
    cmptr->cmsg_type = SCM_RIGHTS;
    cmptr->cmsg_len = sizeof(struct cmsghdr) + sizeof(int);

#  ifdef _XPG4_2
    msg.msg_control = (caddr_t) cmptr;
    msg.msg_controllen = sizeof(struct cmsghdr) + sizeof(int);
    *((int *) CMSG_DATA(cmptr)) = fd;
#  else
    msg.msg_accrights = (caddr_t) &fd;
    msg.msg_accrightslen = sizeof(int);
#  endif
  }

  if(sendmsg(sock(), &msg, 0) != 2)
    return ERRSTREAMSEND;

# else			// Assume Pre-4.4BSD-based

  if(fd < 0) {
    msg.msg_accrights = NULL;
    msg.msg_accrightslen = 0;
  } else {
    msg.msg_accrights = (caddr_t) &fd;
    msg.msg_accrightslen = sizeof(int);
  }

  if(sendmsg(sock(), &msg, 0) != sizeof(buf))
    return ERRSTREAMSEND;

# endif

#endif

  return ERRSTREAMNOERR;
}

short StreamSocket::recvsock_in(StreamSocket *& sockptr)
{
  sockaddr_in peerAddr;
  socklen_t peerSize = sizeof(sockaddr_in);
  int clientSock;
  short err;

  if((err = StreamSocket::recvfd(clientSock)) < 0)
    return err;

  sockaddr_in address;
  socklen_t size = sizeof(sockaddr_in);

  bzero((char *) &address, sizeof(sockaddr_in));

  if(getsockname(clientSock, (sockaddr *) &address, &size) < 0)
    return ERRSTREAMNAME;

  InetAddress clientAddr(address);

#ifndef NDEBUG
  cout << "DEBUG: established connection to client on port "
       << clientAddr.port() << endl;
#endif

  bzero((char *) &peerAddr, sizeof(sockaddr_in));
  size = sizeof(sockaddr_in);

  if(getpeername(clientSock, (sockaddr *) &peerAddr, &size) < 0)
    return ERRSTREAMNAME;

  sockptr = new StreamSocket(clientAddr, clientSock, peerAddr);
  return ERRSTREAMNOERR;
}

short StreamSocket::recvfd(int& filedes)
{
  assert(un_peer_ != 0);

  int newfd, nread, flag, status;
  char *ptr, buf[256];

  status = -1;

#if HAVE_STROPTS_H && HAVE_STREAM_SOCKETS

  struct strbuf dat;
  struct strrecvfd recvfd;

  for(;;) {
    dat.buf = buf;
    dat.maxlen = sizeof(buf);

    flag = 0;

    if(::getmsg(sock(), NULL, &dat, &flag) < 0)
      return ERRSTREAMRECV;

    nread = dat.len;

    if(nread == 0)
      return ERRSTREAMBROKEN;

    for(ptr = buf; ptr < &buf[nread];) {
      if(*ptr++ == 0) {
	if(ptr != &buf[nread - 1])
	  return ERRSTREAMRECV;

	status = *ptr & 255;

	if(status == 0) {
	  if(::ioctl(sock(), I_RECVFD, &recvfd) < 0)
	    return ERRSTREAMRECV;
	  newfd = recvfd.fd;
	}

	nread -= 2;	// Our protocol header is 2 bytes long
      }
    }

    if(status >= 0) {
      filedes = newfd;
      return ERRSTREAMNOERR;
    }
  }

#else			// Most likely BSD-based or SunOS 5.6+ based

# ifdef SCM_RIGHTS	// 4.4BSD-based or SunOS 5.6+ based

  struct iovec iov[1];
  struct msghdr msg;

  char cmbuf[sizeof(struct cmsghdr) + sizeof(int)];
  struct cmsghdr *cmptr = (struct cmsghdr *) cmbuf;

  for(;;) {
    iov[0].iov_base = buf;
    iov[0].iov_len = sizeof(buf);

    msg.msg_iov = iov;
    msg.msg_iovlen = 1;
    msg.msg_name = NULL;
    msg.msg_namelen = 0;

#  ifdef _XPG4_2
    msg.msg_control = (caddr_t) cmptr;
    msg.msg_controllen = sizeof(struct cmsghdr) + sizeof(int);
#  else
    msg.msg_accrights = (caddr_t) &newfd;
    msg.msg_accrightslen = sizeof(int);
#  endif

    if((nread = recvmsg(sock(), &msg, 0)) < 0)
      return ERRSTREAMRECV;
    else if(nread == 0)
      return ERRSTREAMBROKEN;

    for(ptr = buf; ptr < &buf[nread];) {
      if(*ptr++ == 0) {
	if(ptr != &buf[nread - 1])
	  return ERRSTREAMRECV;

	status = *ptr & 255;

#  ifdef _XPG4_2
	if(status == 0) {
	  if(msg.msg_controllen != sizeof(struct cmsghdr) + sizeof(int))
	    return ERRSTREAMRECV;
	  newfd = *(int *) CMSG_DATA(cmptr);
	}
#  else
        if(status == 0) {
          if(msg.msg_accrightslen != sizeof(int))
            return ERRSTREAMRECV;
          newfd = *((int *) msg.msg_accrights);
        }
#  endif

	nread -= 2;	// Our protocol header is 2 bytes
      }
    }

    if(status >= 0) {
      filedes = newfd;
      return ERRSTREAMNOERR;
    }
  }

# else			// Assume Pre-4.4BSD-based

  struct iovec iov[1];
  struct msghdr msg;

  for(;;) {
    iov[0].iov_base = buf;
    iov[0].iov_len = sizeof(buf);

    msg.msg_iov = iov;
    msg.msg_iovlen = 1;
    msg.msg_name = NULL;
    msg.msg_namelen = 0;
    msg.msg_accrights = (caddr_t) &newfd;
    msg.msg_accrightslen = sizeof(int);

    if((nread = recvmsg(sock(), &msg, 0)) < 0)
      return ERRSTREAMRECV;
    else if(nread == 0)
      return ERRSTREAMBROKEN;

    for(ptr = buf; ptr < &buf[nread];) {
      if(*ptr++ == 0) {
	if(ptr != &buf[nread - 1])
	  return ERRSTREAMRECV;

	status = *ptr & 255;

	if(status == 0) {
	  if(msg.msg_accrightslen != sizeof(int))
	    return ERRSTREAMRECV;
	}

	nread -= 2;
      }
    }

    if(status >= 0) {
      filedes = newfd;
      return ERRSTREAMNOERR;
    }
  }

# endif

#endif			// End of #ifdef juju

}
