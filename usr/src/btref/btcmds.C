#include "BTConfig.H" 


#include <iostream>
using namespace std;
#include <signal.h>

#if HAVE_UNISTD_H
# include <unistd.h>
#endif

#if STDC_HEADERS
# include <stdlib.h>
#endif

#include "BTDB.H"
#include "BTDBErr.H"
#include "BTNetworkEntry.H"
#include "BTPlayer.H"
#include "BTPlayerRecord.H"
#include "BTGameStats.H"
#include "BTDirs.H"
#include "BTConfigFile.H"

#include "btcmds.H"
#include "btcmdtab.H"
#include "btref.H"
#include "btglob.H"

extern BTDB *netdb;
extern BTDB *plydb;
extern int prompt;

void cmd_ndata(int argc, char **argv)
{
  if(argc != 1) {
    cerr << "Usage: ndata" << endl;
    return;
  }

  BTNetworkEntry entry;
  short err;

  netdb->rewind();

  if((err = netdb->error()) < 0) {
    cerr << "btref: " << BTDBErrMsg(err) << endl;
    return;
  }

  while(netdb->nextrec(&entry))
    cout << entry << BTCMDS_DELIM;

  cout << flush;
}

void cmd_nlist(int argc, char **argv)
{
  if(argc != 1) {
    cerr << "Usage: nlist" << endl;
    return;
  }

  BTNetworkEntry entry;
  short err;

  netdb->rewind();

  if((err = netdb->error()) < 0) {
    cerr << "btref: " << BTDBErrMsg(err) << endl;
    return;
  }

  while(netdb->nextrec(&entry)) {
    cout << entry.userName_ << '@' << entry.hostName_ << " is ";
    switch(entry.status_) {
    case BTSTATUS_PLAYING:
      cout << "playing\n";
      break;
    case BTSTATUS_WAITING:
      cout << "waiting\n";
      break;
    default:
      cout << "UNKNOWN\n";
    }
  }

  cout << flush;
}

void cmd_ndelete(int argc, char **argv)
{
  if(argc != 2) {
    cerr << "Usage: ndelete <key>" << endl;
    return;
  }

  if(prompt && !confirm())
    return;

  BTNetworkEntry entry;
  short err;

  strncpy(entry.key(), argv[1], BTDBRECORD_KEYLEN);

  if(netdb->remove(&entry)) {
    if(prompt)
      cout << "btref: Network entry deleted" << endl;
  } else {
    if((err = netdb->error()) < 0)
      cerr << "btref: " << BTDBErrMsg(err) << endl;
    else
      cerr << "btref: No network entry with the given key exists" << endl;
  }
}

void cmd_nflush(int argc, char **argv)
{
  if(argc != 1) {
    cerr << "Usage: nflush" << endl;
    return;
  }

  char pathbuf[1024];
  int sendhup = 0;
  pid_t pid;

  if(livedaemon(pid))
    sendhup = 1;

  if(prompt) {
    if(sendhup) {
      cout << "btref: WARNING: Server daemon is currently active" << endl;
      cout << "btref: Referee will send SIGHUP to restart daemons" << endl;
    }
    if(!confirm())
      return;
  }

  BTDB *save = netdb;
  short err;

  strcpy(pathbuf, g_conf->datadir());
  strcat(pathbuf, "/");
  strcat(pathbuf, BTDB_NETWORK);

  netdb = new BTDB(pathbuf, O_CREAT | O_TRUNC | O_RDWR);

  if(!(*netdb)) {
    err = netdb->error();
    cerr << "btref: " << BTDBErrMsg(err) << endl;
    delete netdb;
    netdb = save;
    return;
  }

  delete save;

  if(prompt)
    cout << "btref: Network database flushed of all entries" << endl;

  if(sendhup) {
    if(kill(pid, SIGHUP) < 0)
      cerr << "btref: Failed to send SIGHUP to btserverd" << endl;
    else if(prompt)
      cout << "btref: Sent SIGHUP to btserverd" << endl;
  }
}

void cmd_ncruft(int argc, char **argv)
{
  if(argc != 2) {
    cerr << "Usage: ncruft <threshold>" << endl;
    return;
  }

  int hours = atoi(argv[1]);

  if(hours <= 0) {
    cerr << "btref: Threshold value must be at least 1 hour" << endl;
    return;
  }

  time_t threshold = hours * 60 * 60;
  time_t now = time(0);

  BTNetworkEntry entry;
  short err;

  netdb->rewind();

  if((err = netdb->error()) < 0) {
    cerr << "btref: " << BTDBErrMsg(err) << endl;
    return;
  }

  if(prompt) {
    cout << "Network entries older than " << hours << " hour";
    if(hours > 1) cout << 's';
    cout << ":\n";
  }

  while(netdb->nextrec(&entry))
    if((now - entry.timestamp_) > threshold)
      cout << entry.key() << '\n';

  cout << flush;
}

void cmd_nclean(int argc, char **argv)
{
  if(argc != 2) {
    cerr << "Usage: nclean <threshold>" << endl;
    return;
  }

  int hours = atoi(argv[1]);

  if(hours <= 0) {
    cerr << "btref: Threshold value must be at least 1 hour" << endl;
    return;
  }

  if(prompt && !confirm())
    return;

  time_t threshold = hours * 60 * 60;
  time_t now = time(0);

  BTNetworkEntry entry;
  int cruft = 0;
  short err;

  netdb->rewind();

  if((err = netdb->error()) < 0) {
    cerr << "btref: " << BTDBErrMsg(err) << endl;
    return;
  }

  while(netdb->nextrec(&entry))
    if((now - entry.timestamp_) > threshold)
      cruft++;

  if(!cruft) {
    cout << "btref: No entries older than threshold" << endl;
    return;
  }

  for(int i = 0; i < cruft; i++) {
    netdb->rewind();

    if((err = netdb->error()) < 0) {
      cerr << "btref: " << BTDBErrMsg(err) << endl;
      continue;
    }

    while(netdb->nextrec(&entry))
      if((now - entry.timestamp_) > threshold) {
	if(netdb->remove(&entry)) {
	  cout << "btref: Network entry " << entry.key() << " removed\n";
	  break;
	} else {
	  cerr << "btref: Failed to remove " << entry.key() << '\n';
	  cerr << "btref: " << BTDBErrMsg(netdb->error()) << endl;
	}
      }
  }

  cout << flush;
}

void cmd_ncompress(int argc, char **argv)
{
  if(argc != 1) {
    cerr << "Usage: ncompress" << endl;
    return;
  }

  int sendhup = 0;
  pid_t pid;

  if(livedaemon(pid))
    sendhup = 1;

  if(prompt) {
    cout << "btref: WARNING: Making a backup of files is recommended" << endl;

    if(sendhup) {
      cout << "btref: WARNING: Server daemon is currently active" << endl;
      cout << "btref: Referee will send SIGHUP to restart daemons" << endl;  
    }
 
    if(!confirm())
      return;
  }

  BTNetworkEntry entry;
  char tmpname[1024];
  char newname[1024];
  int compressed = 0;
  short err;

  strcpy(tmpname, g_conf->datadir());
  strcat(tmpname, "/");
  strcat(tmpname, BTCMDS_TMPDB);

  strcpy(newname, g_conf->datadir());
  strcat(newname, "/");
  strcat(newname, BTDB_NETWORK);

  size_t tmplen = strlen(tmpname);
  size_t newlen = strlen(newname);

  BTDB *newdb = new BTDB(tmpname, O_CREAT | O_TRUNC | O_RDWR);

  if(!(*newdb)) {
    cerr << "btref: Failed to initialize new database" << endl;
    err = newdb->error();
    goto dberr;
  }

  netdb->rewind();

  if((err = netdb->error()) < 0)
    goto dberr;

  if(prompt)
    cout << "btref: Compressing network database ...\n" << flush;

  while(netdb->nextrec(&entry))
    if(newdb->insert(&entry)) {
      compressed++;
    } else {
      cerr << "btref: Failed to write record" << endl;
      err = newdb->error();
      goto dberr;
    }

  if((err = netdb->error()) < 0) {
    cerr << "btref: Failed to read all records" << endl;
    goto dberr;
  }

  strcpy((char *) (tmpname + tmplen), BTDB_IDXSUFFIX);
  strcpy((char *) (newname + newlen), BTDB_IDXSUFFIX);
  
  if(::rename(tmpname, newname) < 0) {
    cerr << "btref: Failed to rename compressed index file" << endl;
    err = ERRBTDBNOERR;
    goto dberr;
  }

  strcpy((char *) (tmpname + tmplen), BTDB_DATSUFFIX);
  strcpy((char *) (newname + newlen), BTDB_DATSUFFIX);
  
  if(::rename(tmpname, newname) < 0) {
    cerr << "btref: Failed to rename compressed data file" << endl;
    err = ERRBTDBNOERR;
    goto dberr;
  }

  delete netdb;
  netdb = newdb;

  cout << "btref: " << compressed << " records compressed\n" << flush;

  if(sendhup) {
    if(kill(pid, SIGHUP) < 0)
      cerr << "btref: Failed to send SIGHUP to btserverd" << endl;
    else if(prompt)
      cout << "btref: Sent SIGHUP to btserverd" << endl;
  }

  return;

 dberr:
  cerr << "btref: " << BTDBErrMsg(err) << endl;
  strcpy((char *) (tmpname + tmplen), BTDB_IDXSUFFIX);
  ::unlink(tmpname);
  strcpy((char *) (tmpname + tmplen), BTDB_DATSUFFIX);
  ::unlink(tmpname);
  delete newdb;
}

void cmd_pdata(int argc, char **argv)
{
  if(argc > 2) {
    cerr << "Usage: plist <pattern>" << endl;
    return;
  }

  BTPlayer player;
  short err;

  plydb->rewind();

  if((err = plydb->error()) < 0) {
    cerr << "btref: " << BTDBErrMsg(err) << endl;
    return;
  }

  if(argc == 1) {
    while(plydb->nextrec(&player))
      cout << player.formatInfo() << endl << BTCMDS_DELIM;
  } else {
    if(hasglobchars(argv[1])) {
      while(plydb->nextrec(&player)) {
	if(globmatch(player.key(), argv[1]))
	  cout << player.formatInfo() << endl << BTCMDS_DELIM;
      }
    } else {
      strncpy(player.key(), argv[1], BTDBRECORD_KEYLEN);
      if(plydb->fetch(&player)) {
	cout << player.formatInfo() << endl;
      } else {
	if((err = plydb->error()) < 0)
	  cerr << "btref: " << BTDBErrMsg(err) << endl;
	else
	  cerr << "btref: No such player in database" << endl;
      }
    }
  }

  cout << flush;
}

void cmd_plist(int argc, char **argv)
{
  if(argc > 2) {
    cerr << "Usage: plist <pattern>" << endl;
    return;
  }

  BTPlayer player;
  short err;

  plydb->rewind();

  if((err = plydb->error()) < 0) {
    cerr << "btref: " << BTDBErrMsg(err) << endl;
    return;
  }

  if(argc == 1) {
    while(plydb->nextrec(&player))
      cout << player.key() << '\n';
  } else {
    if(hasglobchars(argv[1])) {
      while(plydb->nextrec(&player)) {
	if(globmatch(player.key(), argv[1]))
	  cout << player.key() << '\n';
      }
    } else {
      strncpy(player.key(), argv[1], BTDBRECORD_KEYLEN);
      if(plydb->fetch(&player)) {
	cout << player.key() << '\n';
      } else {
	if((err = plydb->error()) < 0)
	  cerr << "btref: " << BTDBErrMsg(err) << endl;
	else
	  cerr << "btref: No such player in database" << endl;
      }
    }
  }

  cout << flush;
}

void cmd_pdelete(int argc, char **argv)
{
  if(argc != 2) {
    cerr << "Usage: pdelete pattern" << endl;
    return;
  }

  if(prompt && !confirm())
    return;

  BTPlayer player;
  int deleted = 0;
  short err;

  plydb->rewind();

  if((err = plydb->error()) < 0) {
    cerr << "btref: " << BTDBErrMsg(err) << endl;
    return;
  }

  if(hasglobchars(argv[1])) {

    while(plydb->nextrec(&player)) {
      if(globmatch(player.key(), argv[1])) {
	if(plydb->remove(&player)) {
	  cout << "btref: Removed player " << player.key() << '\n';
	  deleted++;
	} else {
	  if((err = plydb->error()) < 0)
	    cerr << "btref: " << BTDBErrMsg(err) << endl;
	}
      }
    }

    if(!deleted)
      cout << "btref: No records matching pattern were found\n";

  } else {
    strncpy(player.key(), argv[1], BTDBRECORD_KEYLEN);
    if(plydb->remove(&player)) {
      cout << "btref: Removed player " << player.key() << '\n';
    } else {
      if((err = plydb->error()) < 0)
	cerr << "btref: " << BTDBErrMsg(err) << endl;
      else
	cerr << "btref: No such player in database" << endl;
    }
  }

  cout << flush;
}

void cmd_pflush(int argc, char **argv)
{
  if(argc != 1) {
    cerr << "Usage: pflush" << endl;
    return;
  }

  int sendhup = 0;
  pid_t pid;

  if(livedaemon(pid))
    sendhup = 1;

  if(prompt) {
    if(sendhup) {
      cout << "btref: WARNING: Server daemon is currently active" << endl;
      cout << "btref: Referee will send SIGHUP to restart daemons" << endl;
    }
    if(!confirm())
      return;
  }

  BTDB *save = plydb;
  char pathbuf[1024];
  short err;

  strcpy(pathbuf, g_conf->datadir());
  strcat(pathbuf, "/");
  strcpy(pathbuf, BTDB_PLAYERS);

  plydb = new BTDB(pathbuf, O_CREAT | O_TRUNC | O_RDWR);

  if(!(*plydb)) {
    err = plydb->error();
    cerr << "btref: " << BTDBErrMsg(err) << endl;
    delete plydb;
    plydb = save;
    return;
  }

  delete save;

  if(prompt)
    cout << "btref: Player database flushed of all entries" << endl;

  if(sendhup) {
    if(kill(pid, SIGHUP) < 0)
      cerr << "btref: Failed to send SIGHUP to btserverd" << endl;
    else if(prompt)
      cout << "btref: Sent SIGHUP to btserverd" << endl;
  }
}

void cmd_pcompress(int argc, char **argv)
{
  if(argc != 1) {
    cerr << "Usage: pcompress" << endl;
    return;
  }

  int sendhup = 0;
  pid_t pid;

  if(livedaemon(pid))
    sendhup = 1;

  if(prompt) {
    cout << "btref: WARNING: Making a backup of files is recommended" << endl;

    if(sendhup) {
      cout << "btref: WARNING: Server daemon is currently active" << endl;
      cout << "btref: Referee will send SIGHUP to restart daemons" << endl;
    }

    if(!confirm())
      return;
  }

  BTPlayer player;
  char tmpname[1024];
  char newname[1024];
  int compressed = 0;
  short err;

  strcpy(tmpname, g_conf->datadir());
  strcat(tmpname, "/");
  strcat(tmpname, BTCMDS_TMPDB);

  strcpy(newname, g_conf->datadir());
  strcat(newname, "/");
  strcpy(newname, BTDB_PLAYERS);

  size_t tmplen = strlen(tmpname);
  size_t newlen = strlen(newname);

  BTDB *newdb = new BTDB(tmpname, O_CREAT | O_TRUNC | O_RDWR);

  if(!(*newdb)) {
    cerr << "btref: Failed to initialize new database" << endl;
    err = newdb->error();
    goto dberr;
  }

  plydb->rewind();

  if((err = plydb->error()) < 0)
    goto dberr;

  if(prompt)
    cout << "btref: Compressing player database ...\n" << flush;

  while(plydb->nextrec(&player))
    if(newdb->insert(&player)) {
      compressed++;
    } else {
      cerr << "btref: Failed to write record" << endl;
      err = newdb->error();
      goto dberr;
    }

  if((err = plydb->error()) < 0) {
    cerr << "btref: Failed to read all records" << endl;
    goto dberr;
  }

  strcpy((char *) (tmpname + tmplen), BTDB_IDXSUFFIX);
  strcpy((char *) (newname + newlen), BTDB_IDXSUFFIX);
  
  if(::rename(tmpname, newname) < 0) {
    cerr << "btref: Failed to rename compressed index file" << endl;
    err = ERRBTDBNOERR;
    goto dberr;
  }

  strcpy((char *) (tmpname + tmplen), BTDB_DATSUFFIX);
  strcpy((char *) (newname + newlen), BTDB_DATSUFFIX);
  
  if(::rename(tmpname, newname) < 0) {
    cerr << "btref: Failed to rename compressed data file" << endl;
    err = ERRBTDBNOERR;
    goto dberr;
  }

  delete plydb;
  plydb = newdb;

  cout << "btref: " << compressed << " records compressed\n" << flush;

  if(sendhup) {
    if(kill(pid, SIGHUP) < 0)
      cerr << "btref: Failed to send SIGHUP to btserverd" << endl;
    else if(prompt)
      cout << "btref: Sent SIGHUP to btserverd" << endl;
  }

  return;

 dberr:
  cerr << "btref: " << BTDBErrMsg(err) << endl;
  strcpy((char *) (tmpname + tmplen), BTDB_IDXSUFFIX);
  ::unlink(tmpname);
  strcpy((char *) (tmpname + tmplen), BTDB_DATSUFFIX);
  ::unlink(tmpname);
  delete newdb;
}

void cmd_stats(int argc, char **argv)
{
  if(argc != 1) {
    cerr << "Usage: stats" << endl;
    return;
  }

  BTPlayer player;
  short err;

  plydb->rewind();

  if((err = plydb->error()) < 0) {
    cerr << "btref: " << BTDBErrMsg(err) << endl;
    return;
  }

  int players = 0;
  int games = 0;

  if(prompt)
    cout << "Computing statistics ...\n" << flush;

  while(plydb->nextrec(&player)) {
    games += player.wins_ + player.losses_;
    players++;
  }

  cout << "Total players in database: " << players << endl;
  cout << "       Total games played: " << (games / 2) << endl;
}

void cmd_quit(int argc, char **argv)
{
  if(argc != 1) {
    cerr << "Usage: quit" << endl;
    return;
  }

  exit(BTREF_SUCCESS);
}

static int livedaemon(pid_t& pid)
{
  char pathbuf[1024];
  char buf[10];
  int pidfd;

  strcpy(pathbuf, g_conf->datadir());
  strcat(pathbuf, "/");
  strcat(pathbuf, BTMD_PIDFILE);

  if((pidfd = open(pathbuf, O_RDONLY)) < 0)
    return 0;

  buf[0] = '\0';
  read(pidfd, buf, sizeof(buf));
  close(pidfd);
  pid = atoi(buf);

  if(pid && (kill(pid, 0) == 0))
    return 1;

  return 0;
}

static int confirm()
{
  int reply, tmp;

  cout << "Are you sure (y/n) [n] ? " << flush;
  reply = tmp = cin.get();
  while(tmp != '\n' && tmp != '\0' && tmp != EOF)
    tmp = cin.get();

  if((reply == 'y') || (reply == 'Y'))
    return 1;

  cout << "btref: Operation aborted" << endl;
  return 0;
}
