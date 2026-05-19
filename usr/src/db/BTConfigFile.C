#include "BTConfig.H"

#include <sys/stat.h>

#ifdef STAT_MACROS_BROKEN
# define S_ISREG(mode) (((mode) & S_IFMT) == S_IFREG)
# define S_ISDIR(mode) (((mode) & S_IFMT) == S_IFDIR)
#endif


#include <iostream>
using namespace std;

#include "ParsedFile.H"
#include "BTConfigFile.H"

static char *BTCONFIGFILE_DEFPATH = "/";

BTConfigFile::BTConfigFile(const char *configfile)
: status_(BTCONFIGFILE_OK), datadir_(0), logsdir_(0), pipedir_(0), slvpath_(0)
{
  if(configfile == 0) {
    status_ = BTCONFIGFILE_BADFILE;
    return;
  }

  ParsedFile cf(configfile);
  char *varname;

  if(cf.fail()) {
    cerr << "\"" << configfile << "\": Failed to open config file" << endl;
    status_ = BTCONFIGFILE_BADFILE;
    return;
  }

  for(int line = 1; !cf.eof(); line++) {
    cf.parseline();

    if(cf.ntokens() == 0)
      continue;

    if(cf.ntokens() != 2) {
      cerr << "\"" << configfile << "\", line " << line
           << ": Malformed configuration directive." << endl;
      status_ = BTCONFIGFILE_CONFERR;
      return;
    }

    varname = cf.token();

    if(strcmp(varname, "DATADIR") == 0) {
      if(!verifydir(&datadir_, cf.token(), configfile, line))
        datadir_ = BTCONFIGFILE_DEFPATH;
    } else if(strcmp(varname, "LOGSDIR") == 0) {
      if(!verifydir(&logsdir_, cf.token(), configfile, line))
        logsdir_ = BTCONFIGFILE_DEFPATH;
    } else if(strcmp(varname, "PIPEDIR") == 0) {
      if(!verifydir(&pipedir_, cf.token(), configfile, line))
        pipedir_ = BTCONFIGFILE_DEFPATH;
    } else if(strcmp(varname, "SLVPATH") == 0) {
      if(!verifyfile(&slvpath_, cf.token(), configfile, line))
        slvpath_ = BTCONFIGFILE_DEFPATH;
    } else if(strcmp(varname, "AUDIODIR") == 0) {
      if(!verifydir(&audiodir_, cf.token(), configfile, line))
        audiodir_ = BTCONFIGFILE_DEFPATH;
    } else if(strcmp(varname, "ARTDIR") == 0) {
      if(!verifydir(&artdir_, cf.token(), configfile, line))
        artdir_ = BTCONFIGFILE_DEFPATH;
    } else {
      cerr << "\"" << configfile << "\", line " << line
           << ": Invalid configuration variable." << endl;
      status_ = BTCONFIGFILE_CONFERR;
      return;
    }
  }
}

BTConfigFile::~BTConfigFile()
{
  if(datadir_ != BTCONFIGFILE_DEFPATH)
    delete datadir_;
  if(logsdir_ != BTCONFIGFILE_DEFPATH)
    delete logsdir_;
  if(pipedir_ != BTCONFIGFILE_DEFPATH)
    delete pipedir_;
  if(slvpath_ != BTCONFIGFILE_DEFPATH)
    delete slvpath_;
}

BTConfigFile::BTConfigFile(const BTConfigFile& other)
: status_(BTCONFIGFILE_OK), datadir_(0), logsdir_(0), pipedir_(0), slvpath_(0)
{
  if(other.datadir_) {
    if(datadir_ = new char [strlen(other.datadir_) + 1])
      strcpy(datadir_, other.datadir_);
    else
      status_ = BTCONFIGFILE_MEMERR;
  }

  if(other.logsdir_) {
    if(logsdir_ = new char [strlen(other.logsdir_) + 1])
      strcpy(logsdir_, other.logsdir_);
    else
      status_ = BTCONFIGFILE_MEMERR;
  }

  if(other.pipedir_) {
    if(pipedir_ = new char [strlen(other.pipedir_) + 1])
      strcpy(pipedir_, other.pipedir_);
    else
      status_ = BTCONFIGFILE_MEMERR;
  }

  if(other.slvpath_) {
    if(slvpath_ = new char [strlen(other.slvpath_) + 1])
      strcpy(slvpath_, other.slvpath_);
    else
      status_ = BTCONFIGFILE_MEMERR;
  }
}

BTConfigFile& BTConfigFile::operator=(const BTConfigFile& other)
{
  if(this == &other)
    return *this;

  delete datadir_;
  delete logsdir_;
  delete pipedir_;
  delete slvpath_;

  datadir_ = logsdir_ = pipedir_ = slvpath_ = 0;
  status_ = BTCONFIGFILE_OK;

  if(other.datadir_) {
    if(datadir_ = new char [strlen(other.datadir_) + 1])
      strcpy(datadir_, other.datadir_);
    else
      status_ = BTCONFIGFILE_MEMERR;
  }

  if(other.logsdir_) {
    if(logsdir_ = new char [strlen(other.logsdir_) + 1])
      strcpy(logsdir_, other.logsdir_);
    else
      status_ = BTCONFIGFILE_MEMERR;
  }

  if(other.pipedir_) {
    if(pipedir_ = new char [strlen(other.pipedir_) + 1])
      strcpy(pipedir_, other.pipedir_);
    else
      status_ = BTCONFIGFILE_MEMERR;
  }

  if(other.slvpath_) {
    if(slvpath_ = new char [strlen(other.slvpath_) + 1])
      strcpy(slvpath_, other.slvpath_);
    else
      status_ = BTCONFIGFILE_MEMERR;
  }

  return *this;
}

int BTConfigFile::verifyfile(char **bufaddr, const char *token,
                           const char *file, int line)
{
  struct stat sbuf;

  if((*bufaddr = new char [strlen(token) + 1]) == 0) {
    status_ = BTCONFIGFILE_MEMERR;
    return 0;
  }

  strcpy(*bufaddr, token);

  if(stat(token, &sbuf) < 0) {
    cerr << "\"" << file << "\", line " << line
         << ": File " << token << " does not exist." << endl;
    status_ = BTCONFIGFILE_CONFERR;
    return 0;
  }

  if(!S_ISREG(sbuf.st_mode)) {
    cerr << "\"" << file << "\", line " << line
         << ": " << token << " does not refer to a file." << endl;
    status_ = BTCONFIGFILE_CONFERR;
    return 0;
  }

  return 1;
}

int BTConfigFile::verifydir(char **bufaddr, const char *token,
                          const char *file, int line)
{
  struct stat sbuf;

  if((*bufaddr = new char [strlen(token) + 1]) == 0) {
    status_ = BTCONFIGFILE_MEMERR;
    return 0;
  }

  strcpy(*bufaddr, token);

  if(stat(token, &sbuf) < 0) {
    cerr << "\"" << file << "\", line " << line
         << ": Directory " << token << " does not exist." << endl;
    status_ = BTCONFIGFILE_CONFERR;
    return 0;
  }

  if(!S_ISDIR(sbuf.st_mode)) {
    cerr << "\"" << file << "\", line " << line
         << ": " << token << " does not refer to a directory." << endl;
    status_ = BTCONFIGFILE_CONFERR;
    return 0;
  }

  return 1;
}
