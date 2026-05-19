/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: BTSlave.C                                           */
/*    DATE: Wed Oct  5 01:09:04 1994                            */
/****************************************************************/

#include "BTConfig.H"

#if HAVE_UNISTD_H
# include <unistd.h>
#endif

#include <stdio.h>

#include "SigReceiver.H"
#include "StreamSocket.H"
#include "StreamSocketErr.H"

#include "BTDBServer.H"
#include "BTDBErr.H"
#include "BTSDClient.H"
#include "BTSDSignals.H"
#include "BTProtocol.H"
#include "BTDirs.H"
#include "BTConfigFile.H"
#include "BTSlave.H"
#include "btslaved.H"

BTSlave::BTSlave(const char *logpath, int prindex, int maxclients)
: maxclients_(maxclients), log_(logpath, ios::out), awol_(0),
  err_(0)
{
  char pathbuf[1024];
  short err;
  int i;

  sprintf(pathbuf, "%s/%s%d", g_conf->pipedir(), BTMD_PIPENAM, prindex);

  UnixAddress masterAddr(pathbuf);
  UnixAddress slaveAddr;

  log_ << "INFO: btslaved initializing as process-ID " << getpid() << endl;

  if((dbserver_ = new BTDBServer) == 0)
    goto memerr;

  if((err = dbserver_->reload()) < 0) {
    log_ << "ERROR: Failed to load database files" << endl;
    log_ << "ERROR: " << BTDBErrMsg(err) << endl;
    err_ = 1;
    return;
  }

  if((clients_ = new BTSDClient * [maxclients_]) == 0)
    goto memerr;

  for(i = 0; i < maxclients_; i++)
    clients_[i] = 0;

  if((master_ = new StreamSocket(slaveAddr)) == 0)
    goto memerr;

  pbuf_.socket(master_);
  
  timeout_.tv_sec = BTSD_TMOUT_SECS;
  timeout_.tv_usec = BTSD_TMOUT_USECS;

  if((err_ = master_->connect(masterAddr)) < 0)
    goto neterr;

  if((err_ = pbuf_.recvpacket()) < 0)
    goto neterr;

  if(pbuf_.datatype() != BT_OBEY_ME) {
    log_ << "ERROR: Invalid command packet received from master" << endl;
    goto neterr;
  }

  if((err_ = pbuf_.sendpacket(BT_I_OBEY)) < 0)
    goto neterr;

  return;

 memerr:
  log_ << "ERROR: btslaved failed to allocate needed memory" << endl;
  err_ = 1;
  
  if(clients_)
    delete [] clients_;
  if(master_)
    delete master_;
  clients_ = 0;
  master_ = 0;

  return;

 neterr:
  log_ << "ERROR: " << StreamSocketErrMsg(err_) << endl;
  
  if(clients_)
    delete [] clients_;
  if(master_)
    delete master_;
  clients_ = 0;
  master_ = 0;

  return;
}

short BTSlave::run()
{
  if(err_ != 0)
    return err_;

  SigReceiver sigrec;
  sigrec.reset();

  BTSDTermHandler termHdl(this);
  BTSDLst1Handler lst1Hdl(this);
  BTSDLst2Handler lst2Hdl(this);
  BTSDPipeHandler pipeHdl(this);
  BTSDHangHandler hangHdl(this);

  sigrec.install(SIGTERM, &termHdl);
  sigrec.install(SIGINT, &termHdl);
  sigrec.install(SIGQUIT, &termHdl);

  sigrec.install(SIGUSR1, &lst1Hdl);
  sigrec.install(SIGUSR2, &lst2Hdl);
  sigrec.install(SIGPIPE, &pipeHdl);
  sigrec.install(SIGHUP, &hangHdl);

  timeval polltime;
  SELECTARGTYPE set;

  polltime.tv_sec = BTSD_POLL_SECS;
  polltime.tv_usec = BTSD_POLL_USECS;

  for(;;) {
    if((err_ = dbserver_->update()) < 0)
      return err_;

    if(awol_)
      maxfd_ = -1;
    else
      maxfd_ = master_->sock();

    FD_ZERO(&set);
    FD_SET(master_->sock(), &set);

    for(int i = 0; i < maxclients_; i++) {
      if(clients_[i] != 0) {
	if(clients_[i]->sockfd() > maxfd_)
	  maxfd_ = clients_[i]->sockfd();
	FD_SET(clients_[i]->sockfd(), &set);
      }
    }

    if((maxfd_ < 0) && awol_) {
      log_ << "INFO: Terminating because we're AWOL and clientless" << endl;
      terminate();
      exit(0);
    }

    if(select(maxfd_ + 1, (SELECTARGTYPE *) &set, (SELECTARGTYPE *) 0,
              (SELECTARGTYPE *) 0, &polltime) > 0) {

      if(FD_ISSET(master_->sock(), &set)) {
	if(BTSlave::obey() == ERRSTREAMBROKEN)
	  awol_ = 1;
      }

      for(int i = 0; i < maxclients_; i++) {
	if((clients_[i] != 0) && FD_ISSET(clients_[i]->sockfd(), &set))
	  BTSlave::process(i);
      }
    }

    if((err_ = dbserver_->processq()) < 0)
      return err_;
  }
}

void BTSlave::disconnect()
{
  log_ << "ERROR: Disconnected from master\n" << flush;
  awol_ = 1;
}

void BTSlave::terminate()
{
  log_ << "INFO: Closing connection to all clients" << endl;

  if(clients_) {
    for(int i = 0; i < maxclients_; i++)
      delete clients_[i];
    delete [] clients_;
  }

  log_ << "INFO: Closing database server" << endl;
  delete dbserver_;

  log_ << "INFO: Closing connection to master" << endl;
  delete master_;

  log_ << "INFO: Slave terminating\n" << flush;
}

short BTSlave::obey()
{
  log_ << "INFO: Receiving request from master\n" << flush;

  StreamSocket *clientSock = 0;
  BTSDClient *client = 0;

  short err;
  int i;

  if((err = pbuf_.recvpacket()) < 0) {
    log_ << "ERROR: [master] " << StreamSocketErrMsg(err) << endl;
    return err;
  }

  if(pbuf_.datatype() != BT_NEWCLIENT) {
    log_ << "ERROR: [master] Invalid command packet received" << endl;
    return err;
  }

  log_ << "INFO: [master] Received BT_NEWCLIENT command packet" << endl;

  for(i = 0; i < maxclients_; i++) {
    if(clients_[i] == 0)
      break;
  }

  if(i == maxclients_) {
    log_ << "INFO: BT_NEWCLIENT failed because client array is full" << endl;
    if((err = pbuf_.sendpacket(BT_CLIENTBAD)) < 0)
      log_ << "ERROR: [master] " << StreamSocketErrMsg(err) << endl;
    return err;
  } else {
    if((err = pbuf_.sendpacket(BT_CLIENTOK)) < 0) {
      log_ << "ERROR: [master] " << StreamSocketErrMsg(err) << endl;
      return err;
    }
  }

  if((err = master_->recvsock_in(clientSock)) < 0) {
    log_ << "ERROR: [master] " << StreamSocketErrMsg(err) << endl;
    return err;
  }

  if(clientSock == 0) {
    log_ << "ERROR: Insufficient memory to create new stream socket" << endl;
    return err;
  }

  if((client = new BTSDClient(clientSock)) == 0) {
    log_ << "ERROR: Insufficient memory to create new client object" << endl;
    return err;
  }

  if(client->error() < 0) {
    log_ << "ERROR: [client] " << StreamSocketErrMsg(client->error()) << endl;
    delete client;
    return err;
  }

  if((err = dbserver_->insert(client)) < 0) {
    log_ << "ERROR: [db] " << BTDBErrMsg(err) << endl;
    delete client;
    return ERRSTREAMNOERR;
  }

  log_ << "INFO: Client: " << *client << endl << flush;
  clients_[i] = client;
  return ERRSTREAMNOERR;
}

void BTSlave::process(int idx)
{
  BTNetworkEntry nentry;
  BTGameStats stats;

  unsigned long entries;
  unsigned short valid;
  short err;

  BTSDClient *client = clients_[idx];
  cbuf_.socket(client->sock());

  if((err = cbuf_.recvpacket()) < 0)
    goto neterr;

  switch(cbuf_.datatype()) {

  case BT_QUER_NETDB:
    entries = htonl(dbserver_->netentries());
    if((err = cbuf_.sendpacket(BT_RESP_DBLEN, sizeof(entries),
			       (char *) &entries)) < 0)
      goto neterr;
    if((err = cbuf_.sendpacket(BT_RESP_NETDB, dbserver_->netlen(),
			       dbserver_->netbuf())) < 0)
      goto neterr;
    log_ << "INFO: Processed BT_QUER_NETDB from " << *client << endl;
    break;

  case BT_QUER_PLYDB:
    entries = htonl(dbserver_->plyentries());
    if((err = cbuf_.sendpacket(BT_RESP_DBLEN, sizeof(entries),
			       (char *) &entries)) < 0)
      goto neterr;
    if((err = cbuf_.sendpacket(BT_RESP_PLYDB, dbserver_->plylen(),
			       dbserver_->plybuf())) < 0)
      goto neterr;
    log_ << "INFO: Processed BT_QUER_PLYDB from " << *client << endl;
    break;

  case BT_QUER_VERIFY:
    nentry.readbuf(cbuf_.databuf());
    valid = htons((unsigned short) dbserver_->verify(nentry));
    if((err = cbuf_.sendpacket(BT_RESP_VERIFY, sizeof(valid),
			       (char *) &valid)) < 0)
      goto neterr;
    log_ << "INFO: Processed BT_QUER_VERIFY from " << *client << endl;
    break;

  case BT_QUER_UPDATE:
    if((err = dbserver_->modify(client)) < 0)
      goto dberr;
    log_ << "INFO: Processed BT_REQ_UPDATE from " << *client << endl;
    break;

  case BT_QUER_RESULT:
    stats.readbuf(cbuf_.databuf());
    dbserver_->enqueue(stats);
    log_ << "INFO: Processed BT_QUER_RESULT from " << *client << endl;
    break;

  case BT_DISCONNECT:
    dbserver_->revoke(client);
    delete client;
    clients_[idx] = 0;
    log_ << "INFO: Processed BT_DISCONNECT from " << *client << endl;
    break;

  default:
    log_ << "ERROR: Invalid request packet received from " << *client << endl;
  }

  return;

 neterr:
  log_ << "ERROR: [client] " << StreamSocketErrMsg(err) << endl;

  if(err == ERRSTREAMBROKEN) {
    log_ << "ERROR: Connection broke with " << *client << endl;
    dbserver_->revoke(client);
    delete client;
    clients_[idx] = 0;
  } else {
    log_ << "ERROR: Failed to process request from " << *client << endl;
  }

  return;

 dberr:
  log_ << "ERROR: [db] " << BTDBErrMsg(err) << endl;
  log_ << "ERROR: Failed to process request from " << *client << endl;
  return;
}

void BTSlave::listClients()
{
  log_ << "INFO: Current clients:\n";

  for(int i = 0; i < maxclients_; i++) {
    if(clients_[i])
      log_ << "INFO: " << *(clients_[i]) << endl;
    else
      log_ << "INFO: No client in slot " << i << endl;
  }

  log_ << flush;
}

void BTSlave::listQueue()
{
  log_ << "INFO: Current request queue:\n";
  log_ << *dbserver_;
  log_ << flush;
}

void BTSlave::restart()
{
  log_ << "INFO: Received hangup request" << endl << flush;
  dbserver_->restart();
}
