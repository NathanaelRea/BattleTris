/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: BTServer.C                                          */
/*    DATE: Tue Sep 27 23:32:41 1994                            */
/****************************************************************/

#include "BTConfig.H"

#if HAVE_UNISTD_H
# include <unistd.h>
#endif

#include <sys/stat.h>

#if HAVE_SYS_WAIT_H
# include <sys/wait.h>
#endif

#ifndef WEXITSTATUS
# define WEXITSTATUS(status) ((unsigned) (status) >> 8)
#endif
#ifndef WIFEXITED
# define WIFEXITED(status) ((status & 255) == 0)
#endif

#include <stdio.h>
#include <errno.h>
#include <fcntl.h>

#include "SigReceiver.H"
#include "StreamSocket.H"
#include "StreamSocketErr.H"

#include "BTDirs.H"
#include "BTConfigFile.H"
#include "BTConstants.H"
#include "BTProtocol.H"
#include "BTMDSignals.H"
#include "BTMDSlave.H"
#include "BTServer.H"
#include "btserverd.H"

const int NUMBINDTRIES = 8;	// Number of bind attempts before failing
const int BINDNAPTIME = 30;	// Sleep time between each bind attempt

BTServer::BTServer(int nslaves, int port)
: nslaves_(nslaves), death_(0), sidx_(0), err_(0), enabled_(0)
{
  pid_t mypid = getpid();
  int pidfd, oldpid, i;

  char pathbuf[1024];
  char buf[16];

  UnixAddress addr;
  char *addrpath;

  strcpy(pathbuf, g_conf->logsdir());
  strcat(pathbuf, "/");
  strcat(pathbuf, BTMD_LOGFILE);

  if((log_ = new ofstream(pathbuf, ios::out)) == 0)
    goto memerr;

  if((pidfile_ =
        new char [strlen(g_conf->datadir()) + strlen(BTMD_PIDFILE) + 1]) == 0)
    goto memerr;

  // Allocate 16 extra bytes so we can concatenate the daemon prindex
  // and a delimeter onto the end of this buffer.

  if((addrpath =
        new char [strlen(g_conf->pipedir()) + strlen(BTMD_PIPENAM) + 16]) == 0)
    goto memerr;

  strcpy(pidfile_, g_conf->datadir());
  strcat(pidfile_, "/");
  strcat(pidfile_, BTMD_PIDFILE);

  if((pidfd = open(pidfile_, O_CREAT | O_EXCL | O_WRONLY, 0660)) < 0) {
    if((pidfd = open(pidfile_, O_RDONLY)) < 0) {
      cerr << "btserverd: Failed to open process-ID file" << endl;
      err_ = 1;
      return;
    }

    buf[0] = '\0';
    read(pidfd, buf, sizeof(buf));
    oldpid = atoi(buf);

    if(kill(oldpid, 0) < 0) {
      if(errno != ESRCH) {
        cerr << "btserverd: Server already running as pid " << oldpid << endl;
        err_ = 1;
        return;
      }
    } else {
      cerr << "btserverd: Server already running as pid " << oldpid << endl;
      err_ = 1;
      return;
    }
  }

  sprintf(buf, "%d", mypid);
  write(pidfd, buf, strlen(buf) + 1);
  close(pidfd);

  *log_ << "INFO: BattleTris Server initializing" << endl;
  *log_ << "INFO: Current process-id is " << mypid << endl;

  {
    InetAddress laddr((unsigned short) port);
    if((listener_ = new StreamSocket(laddr)) == 0)
      goto memerr;
  }

  if((sockets_ = new StreamSocket * [nslaves_]) == 0)
    goto memerr;

  if((slaves_ = new BTMDSlave * [nslaves_]) == 0)
    goto memerr;

  bzero((char *) sockets_, sizeof(StreamSocket *) * nslaves_);
  bzero((char *) slaves_, sizeof(BTMDSlave *) * nslaves_);

  *log_ << "INFO: Spawning " << nslaves_ << " slave daemons" << endl;

  for(i = 0; i < nslaves; i++) {
    sprintf(addrpath, "%s/%s%d", g_conf->pipedir(), BTMD_PIPENAM, i);

    unlink(addrpath);
    addr.path(addrpath);

    if((sockets_[i] = new StreamSocket(addr)) == 0)
      goto memerr;

    if((err_ = sockets_[i]->listen(SOMAXCONN)) < 0) {
      cerr << "btserverd: " << StreamSocketErrMsg(err_) << endl;
      *log_ << "ERROR: " << StreamSocketErrMsg(err_) << endl;
      goto neterr;
    }

    spawnSlave(i);

    if(slaves_[i] == 0) {
      cerr << "btserverd: Failed to spawn slave daemon" << endl;
      err_ = 1;
      goto neterr; 
    }
  }

  delete [] addrpath;
  return;

 memerr:
  if(log_) {
    *log_ << "ERROR: btserverd failed to allocate needed memory" << endl;
    *log_ << "ERROR: btserverd failed to initialize properly" << endl;
  } else {
    cerr << "ERROR: btserverd failed to allocate needed memory" << endl;
    cerr << "ERROR: btserverd failed to initialize properly" << endl;
  }
  err_ = 1;
  return;

 neterr:
  *log_ << "ERROR: btserverd failed to initialize properly" << endl;
  err_ = 1;
  return;
}

BTServer::~BTServer()
{
  killSlaves();

  if(listener_)
    delete listener_;

  if(pidfile_)
    delete [] pidfile_;

  if(log_)
    delete log_;
}

short BTServer::run()
{
  if(err_)
    return err_;

  SigReceiver sigrec;
  sigrec.reset();

  BTMDTermHandler termHdl(this);
  BTMDListHandler listHdl(this);
  BTMDSuspHandler suspHdl(this);
  BTMDChldHandler chldHdl(this);
  BTMDHangHandler hangHdl(this);

  sigrec.install(SIGTERM, &termHdl);
  sigrec.install(SIGINT, &termHdl);

  sigrec.install(SIGUSR1, &listHdl);
  sigrec.install(SIGUSR2, &suspHdl);
  sigrec.install(SIGHUP, &hangHdl);

  sigrec.disable(SIGQUIT);
  sigrec.disable(SIGPIPE);

  StreamSocket *client;
  SELECTARGTYPE set;

  for(int i = 0; i < NUMBINDTRIES; i++) {
    if((err_ = listener_->listen(SOMAXCONN)) < 0) {
      if(err_ == ERRSTREAMBIND) {
        *log_ << "INFO: Failed to bind: " << strerror(errno)
              << " ... sleeping ..." << endl;
        sleep(BINDNAPTIME);
      } else {
        cerr << "btserverd: " << StreamSocketErrMsg(err_) << endl;
        return err_;
      }
    } else {
      break;
    }
  }

  if(err_ != ERRSTREAMNOERR) {
    cerr << "btserverd: " << StreamSocketErrMsg(err_) << endl;
    return err_;
  }

  sigrec.install(SIGCHLD, &chldHdl);

  for(enabled_ = 1;;) {
    if(!enabled_) {
      pause();
      continue;
    }

    FD_ZERO(&set);
    FD_SET(listener_->sock(), &set);

    if(select(listener_->sock() + 1, (SELECTARGTYPE *) &set,
              (SELECTARGTYPE *) 0, (SELECTARGTYPE *) 0,
              (struct timeval *) 0) < 0)
      continue;

    if(FD_ISSET(listener_->sock(), &set)) {
      if(listener_->accept(client) < 0)
	continue;

      pbuf_.socket(client);

      if(acceptClient(client))
	pbuf_.sendpacket(BT_ACCEPTED);
      else
	pbuf_.sendpacket(BT_REJECTED);

      delete client;
    }
  }

  return err_;
}

void BTServer::toggleListener()
{
  if(enabled_ = 1 - enabled_)
    *log_ << "INFO: Listening for incoming connections\n" << flush;
  else
    *log_ << "INFO: Disabling listening for connections\n" << flush;
}

void BTServer::restartSlaves()
{
  for(int i = 0; i < nslaves_; i++) {
    if(slaves_[i]) {
      kill(slaves_[i]->pid(), SIGHUP);
      *log_ << "INFO: Forwarded hangup to slave process "
	   << slaves_[i]->pid() << endl;
    }
  }

  *log_ << flush;
}

void BTServer::listSlaves()
{
  *log_ << "INFO: Listing slave process information\n";

  for(int i = 0; i < nslaves_; i++) {
    if(slaves_[i])
      *log_ << "INFO: " << *(slaves_[i]);
    else
      *log_ << "INFO: No slave in slot " << i << endl;
  }

  *log_ << flush;
}

void BTServer::killSlaves()
{
  char addrpath[1024];

  *log_ << "INFO: Closing network connection" << endl;

  if(listener_)
    delete listener_;

  *log_ << "INFO: Terminating slave daemons" << endl;
  death_ = 1;

  if(slaves_) {
    for(int i = 0; i < nslaves_; i++)
      delete slaves_[i];

    delete [] slaves_;
  }

  *log_ << "INFO: Closing UNIX domain sockets" << endl;

  if(sockets_) {
    for(int i = 0; i < nslaves_; i++) {
      if(sockets_[i])
        delete sockets_[i];

      sprintf(addrpath, "%s/%s%d", g_conf->pipedir(), BTMD_PIPENAM, i);
      unlink(addrpath);
    }

    delete [] sockets_;
  }

  *log_ << "INFO: Server terminated" << endl << flush;
  unlink(pidfile_);
}

void BTServer::spawnSlave(int prindex)
{
  if((slaves_[prindex] = new BTMDSlave(sockets_[prindex], prindex)) == 0) {
    *log_ << "ERROR: Insufficient memory for new daemon object" << endl;
    return;
  }

  if(slaves_[prindex]->error() < 0) {
    *log_ << "ERROR: " << StreamSocketErrMsg(slaves_[prindex]->error()) << endl;
    delete slaves_[prindex];
    slaves_[prindex] = 0;
  } else if(slaves_[prindex]->error() > 0) {
    *log_ << "ERROR: Failed to fork/exec/connect new slave process" << endl;
    delete slaves_[prindex];
    slaves_[prindex] = 0;
  } else {
    *log_ << "INFO: Spawned slave process " << slaves_[prindex]->pid()
	 << " in slot " << prindex << endl;
  }
}

void BTServer::deadSlave()
{
  int status;
  pid_t cpid;

  if((cpid = wait(&status)) == -1) {
    *log_ << "ERROR: Failed to wait following SIGCHLD\n" << flush;
    return;
  }

  if(WIFEXITED(status)) {
    *log_ << "INFO: Slave process " << cpid << " terminated normally" << endl;
    *log_ << "INFO: Slave returned " << WEXITSTATUS(status) << endl;
  } else {
    if(death_)
      *log_ << "INFO: ";
    else
      *log_ << "ERROR: ";
    *log_ << "Slave process " << cpid << " terminated by signal" << endl;
  }

  if(!death_) {

    int slot = -1;

    for(int i = 0; i < nslaves_; i++)
      if(slaves_[i]->pid() == cpid) {
	delete slaves_[i];
	slot = i;
	break;
      }

    if(slot < 0) {
      *log_ << "ERROR: Slave process " << cpid << " was not in table" << endl;
      return;
    }	

    spawnSlave(slot);
    if(slaves_[slot] == 0)
      *log_ << "ERROR: Failed to respawn daemon in slot " << slot << endl;
  }
}

int BTServer::acceptClient(StreamSocket *client)
{
  short err;

  *log_ << "INFO: Processing connection request" << endl;
  for(int i = 0; i < nslaves_; i++) {
    if((err = slaves_[sidx_]->acceptClient(client)) < 0) {
      *log_ << "ERROR: [slave " << sidx_ << "]: "
           << StreamSocketErrMsg(err) << endl;
      return 0;
    } else if(err > 0) {
      *log_ << "INFO: Slave " << sidx_ << " has accepted the client" << endl;
      sidx_ = (sidx_ + 1) % nslaves_;
      return 1;
    }

    sidx_ = (sidx_ + 1) % nslaves_;
  }

  *log_ << "ERROR: No slaves were able to accept the client" << endl;
  return 0;
}
